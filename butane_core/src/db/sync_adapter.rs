use crate::db::RawQueryResult;
use crate::migrations::adb;
use crate::query::{BoolExpr, Order};
use crate::{Column, Result, SqlVal, SqlValRef};

use std::future::Future;

#[derive(Debug)]
pub struct SyncAdapter<T> {
    runtime_handle: tokio::runtime::Handle,
    _runtime: Option<tokio::runtime::Runtime>,
    inner: T,
}

impl<T> SyncAdapter<T> {
    // TODO needs an inner new that preserves the handle
    pub fn new(inner: T) -> Result<Self> {
        // TODO needs to check that the existing runtime isn't a current_thread
        // if it is, handle.block_on can't drive IO.
        // We can create a new runtime in that case, but not on the same thread.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => Ok(Self {
                runtime_handle: handle,
                _runtime: None,
                inner,
            }),
            Err(_) => {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()?;
                Ok(Self {
                    runtime_handle: runtime.handle().clone(),
                    _runtime: Some(runtime),
                    inner,
                })
            }
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

impl<T> crate::db::sync::ConnectionMethods for SyncAdapter<T>
where
    T: crate::db::ConnectionMethods,
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

impl<T> crate::db::sync::BackendConnection for SyncAdapter<T>
where
    T: crate::db::BackendConnection,
{
    fn transaction(&mut self) -> Result<crate::db::sync::Transaction<'_>> {
        let transaction: crate::db::Transaction =
            self.runtime_handle.block_on(self.inner.transaction())?;
        let transaction_adapter = SyncAdapter::new(transaction.trans)?;
        Ok(crate::db::sync::Transaction::new(Box::new(
            transaction_adapter,
        )))
    }
    fn backend(&self) -> Box<dyn crate::db::sync::Backend> {
        Box::new(SyncAdapter::new(self.inner.backend()).unwrap())
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
    T: crate::db::BackendConnection + 'static,
{
    pub fn into_connection(self) -> crate::db::sync::Connection {
        crate::db::sync::Connection::new(Box::new(self))
    }
}

impl<'c, T> crate::db::sync::BackendTransaction<'c> for SyncAdapter<T>
where
    T: crate::db::BackendTransaction<'c>,
{
    fn commit(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.commit())
    }
    fn rollback(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.rollback())
    }
    fn connection_methods(&self) -> &dyn crate::db::sync::ConnectionMethods {
        self
    }
}

impl<T> crate::db::sync::Backend for SyncAdapter<T>
where
    T: crate::db::Backend + Clone,
{
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String> {
        self.inner.create_migration_sql(current, ops)
    }
    fn connect(&self, conn_str: &str) -> Result<crate::db::sync::Connection> {
        let conn_async = self.block_on(self.inner.connect(conn_str))?;
        Ok(crate::db::sync::Connection {
            conn: Box::new(SyncAdapter::new(conn_async.conn)?),
        })
    }
}
