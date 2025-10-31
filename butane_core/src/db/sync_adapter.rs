use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;

use crate::db::{
    Backend, BackendConnection, BackendConnectionAsync, BackendTransaction,
    BackendTransactionAsync, Connection, ConnectionAsync, ConnectionMethods, RawQueryResult,
    Transaction, TransactionAsync,
};
use crate::migrations::adb;
use crate::query::{BoolExpr, Order};
use crate::{debug, Column, Result, SqlVal, SqlValRef};

/// Adapter that allows running synchronous operations on an async type.
#[derive(Debug)]
pub struct SyncAdapter<T> {
    runtime_handle: tokio::runtime::Handle,
    _runtime: Option<Arc<tokio::runtime::Runtime>>,
    inner: T,
}

impl<T> SyncAdapter<T> {
    pub fn new(inner: T) -> Result<Self> {
        // TODO needs to check that the existing runtime isn't a current_thread
        // if it is, handle.block_on can't drive IO.
        // We can create a new runtime in that case, but not on the same thread.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                debug!("Using existing tokio runtime");
                Ok(Self {
                    runtime_handle: handle,
                    _runtime: None,
                    inner,
                })
            }
            Err(_) => {
                debug!("Creating new tokio runtime");
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()?;
                Ok(Self {
                    runtime_handle: runtime.handle().clone(),
                    _runtime: Some(Arc::new(runtime)),
                    inner,
                })
            }
        }
    }

    /// Create a [`SyncAdapter`] for a different type, using the same runtime.
    fn chain<S>(&self, inner: S) -> SyncAdapter<S> {
        SyncAdapter {
            runtime_handle: self.runtime_handle.clone(),
            _runtime: self._runtime.as_ref().cloned(),
            inner,
        }
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime_handle.block_on(future)
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Clone for SyncAdapter<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        SyncAdapter {
            runtime_handle: self.runtime_handle.clone(),
            _runtime: None,
            inner: self.inner.clone(),
        }
    }
}

impl<T> crate::db::ConnectionMethods for SyncAdapter<T>
where
    T: crate::db::ConnectionMethodsAsync,
{
    fn execute(&self, sql: &str) -> Result<()> {
        self.block_on(self.inner.execute(sql))
    }
    fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.block_on(self.inner.query(table, columns, expr, limit, offset, sort))
    }
    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.block_on(
            self.inner
                .insert_returning_pk(table, columns, pkcol, values),
        )
    }
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        self.block_on(self.inner.insert_only(table, columns, values))
    }
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.block_on(self.inner.insert_or_replace(table, columns, pkcol, values))
    }
    fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.block_on(self.inner.update(table, pkcol, pk, columns, values))
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.block_on(self.inner.delete_where(table, expr))
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        self.block_on(self.inner.has_table(table))
    }
}

impl<T> BackendConnection for SyncAdapter<T>
where
    T: BackendConnectionAsync,
{
    fn transaction(&mut self) -> Result<Transaction<'_>> {
        // We can't use chain because of the lifetimes and mutable borrows below,
        // so set up these runtime clones now.
        let runtime_handle = self.runtime_handle.clone();
        let runtime = self._runtime.as_ref().cloned();
        let transaction: TransactionAsync =
            self.runtime_handle.block_on(self.inner.transaction())?;
        let transaction_adapter = SyncAdapter {
            runtime_handle,
            _runtime: runtime,
            inner: transaction.trans,
        };
        Ok(Transaction::new(Box::new(transaction_adapter)))
    }
    fn backend(&self) -> Box<dyn crate::db::Backend> {
        self.inner.backend()
    }
    fn backend_name(&self) -> &'static str {
        self.inner.backend_name()
    }
    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

impl<T> SyncAdapter<T>
where
    T: BackendConnectionAsync + 'static,
{
    pub fn into_connection(self) -> Connection {
        Connection::new(Box::new(self))
    }
}

impl<'c, T> BackendTransaction<'c> for SyncAdapter<T>
where
    T: BackendTransactionAsync<'c>,
{
    fn commit(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.commit())
    }
    fn rollback(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.rollback())
    }
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

#[async_trait]
impl<T> Backend for SyncAdapter<T>
where
    T: Backend + Clone,
{
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn row_id_column(&self) -> Option<&'static str> {
        self.inner.row_id_column()
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String> {
        self.inner.create_migration_sql(current, ops)
    }
    fn connect(&self, conn_str: &str) -> Result<Connection> {
        let conn_async = self.block_on(self.inner.connect_async(conn_str))?;
        let conn = Connection {
            conn: Box::new(self.chain(conn_async.conn)),
        };
        Ok(conn)
    }
    async fn connect_async(&self, conn_str: &str) -> Result<ConnectionAsync> {
        self.inner.connect_async(conn_str).await
    }
}
