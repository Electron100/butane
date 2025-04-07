//! Types, traits, and methods for interacting with a database.
//!
//! The different ways of referring to a database handle may present
//! some initial confusion.
//! * `ConnectionMethods` is a trait containing the methods available on a database connection or a transaction.
//!   Most methods on a [DataObject][crate::DataObject] or a [Query][crate::query::Query] require an
//!   implementation of `ConnectionMethods`.
//! * `BackendConnection` is a trait representing a direct connection to a database backend. It is a superset
//!   of `ConnectionMethods` and also includes the ability to create a transaction.
//! * `Transaction` is a struct representing a database transaction. It implements `ConnectionMethods`.
//! * `Connection` is a convenience struct containing a boxed `BackendConnection`. It cannot do anything other than
//!   what a `BackendConnection` can do, but allows using a single concrete type that is not tied to a particular
//!   database backend. It is returned by the `connect` method.

#![allow(missing_docs)]

use std::borrow::Cow;
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use async_trait::async_trait;
use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};

use crate::query::{BoolExpr, Order};
use crate::{migrations::adb, Error, Result, SqlVal, SqlValRef};

#[cfg(feature = "async-adapter")]
mod adapter;
#[cfg(feature = "async-adapter")]
pub use adapter::connect_async_via_sync;
#[cfg(feature = "async")]
pub(crate) mod dummy;

#[cfg(feature = "async")]
mod sync_adapter;
#[cfg(feature = "async")]
pub use sync_adapter::SyncAdapter;

mod connmethods;
#[cfg(feature = "async")]
pub use connmethods::ConnectionMethodsAsync;
pub use connmethods::{
    BackendRow, BackendRows, Column, ConnectionMethods, MapDeref, QueryResult, RawQueryResult,
};
mod helper;
mod macros;
#[cfg(feature = "pg")]
pub mod pg;

#[cfg(feature = "sqlite")]
pub mod sqlite;

// Macros are always exported at the root of the crate
use crate::connection_method_wrapper;

mod internal {
    // AsyncRequiresSend and AsyncRequiresSync are used to conditionally add bounds
    // to types only in their async version.

    #[maybe_async_cfg::maybe(sync())]
    pub trait AsyncRequiresSend {}
    #[maybe_async_cfg::maybe(idents(AsyncRequiresSend), sync())]
    impl<T> AsyncRequiresSend for T {}

    #[maybe_async_cfg::maybe(async(feature = "async"))]
    pub trait AsyncRequiresSend: Send {}
    #[maybe_async_cfg::maybe(idents(AsyncRequiresSend), async(feature = "async"))]
    impl<T: Send> AsyncRequiresSend for T {}

    #[maybe_async_cfg::maybe(sync())]
    pub trait AsyncRequiresSync {}
    #[maybe_async_cfg::maybe(idents(AsyncRequiresSync), sync())]
    impl<T> AsyncRequiresSync for T {}

    #[maybe_async_cfg::maybe(async(feature = "async"))]
    pub trait AsyncRequiresSync: Sync {}
    #[maybe_async_cfg::maybe(idents(AsyncRequiresSync), async(feature = "async"))]
    impl<T: Sync> AsyncRequiresSync for T {}
}

/// Database connection.
#[maybe_async_cfg::maybe(
    idents(
        ConnectionMethods(sync = "ConnectionMethods", async = "ConnectionMethodsAsync"),
        Transaction(sync = "Transaction", async = "TransactionAsync"),
    ),
    sync(self = "BackendConnection"),
    async(feature = "async", self = "BackendConnectionAsync")
)]
#[async_trait]
pub trait BackendConnection: ConnectionMethods + Debug + Send {
    /// Begin a database transaction. The transaction object must be
    /// used in place of this connection until it is committed or aborted.
    async fn transaction(&mut self) -> Result<Transaction<'_>>;
    /// Retrieve the backend for this connection.
    fn backend(&self) -> Box<dyn Backend>;
    /// Retrieve the backend name for this connection.
    fn backend_name(&self) -> &'static str;
    /// Tests if the connection has been closed. Backends which do not
    /// support this check should return false.
    fn is_closed(&self) -> bool;
}

#[maybe_async_cfg::maybe(
    idents(
        BackendConnection(sync = "BackendConnection"),
        Connection(sync = "Connection"),
        Transaction(sync = "Transaction")
    ),
    keep_self,
    sync(),
    async(feature = "async")
)]
#[async_trait]
impl BackendConnection for Box<dyn BackendConnection> {
    async fn transaction(&mut self) -> Result<Transaction> {
        self.deref_mut().transaction().await
    }
    fn backend(&self) -> Box<dyn Backend> {
        self.deref().backend()
    }
    fn backend_name(&self) -> &'static str {
        self.deref().backend_name()
    }
    fn is_closed(&self) -> bool {
        self.deref().is_closed()
    }
}

#[maybe_async_cfg::maybe(
    idents(
        BackendConnection(sync = "BackendConnection"),
        ConnectionMethods(sync = "ConnectionMethods")
    ),
    keep_self,
    sync(),
    async(feature = "async")
)]
#[async_trait]
impl ConnectionMethods for Box<dyn BackendConnection> {
    async fn execute(&self, sql: &str) -> Result<()> {
        self.deref().execute(sql).await
    }
    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.deref()
            .query(table, columns, expr, limit, offset, sort)
            .await
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.deref()
            .insert_returning_pk(table, columns, pkcol, values)
            .await
    }
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref().insert_only(table, columns, values).await
    }
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref()
            .insert_or_replace(table, columns, pkcol, values)
            .await
    }
    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref().update(table, pkcol, pk, columns, values).await
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.deref().delete_where(table, expr).await
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        self.deref().has_table(table).await
    }
}

/// Database connection. May be a connection to any type of database
/// as it is a boxed abstraction over a specific connection.
#[maybe_async_cfg::maybe(
    idents(BackendConnection(sync = "BackendConnection")),
    sync(keep_self),
    async(self = "ConnectionAsync", feature = "async")
)]
#[derive(Debug)]
pub struct Connection {
    conn: Box<dyn BackendConnection>,
}

#[maybe_async_cfg::maybe(
    idents(BackendConnection(sync = "BackendConnection")),
    sync(keep_self),
    async(feature = "async")
)]
impl Connection {
    pub fn new(conn: Box<dyn BackendConnection>) -> Self {
        Self { conn }
    }
    pub async fn execute(&self, sql: impl AsRef<str>) -> Result<()> {
        self.conn.execute(sql.as_ref()).await
    }
    // For use with connection_method_wrapper macro.
    #[allow(clippy::unnecessary_wraps)]
    fn wrapped_connection_methods(&self) -> Result<&dyn BackendConnection> {
        Ok(self.conn.as_ref())
    }

    /// Consume this connection and convert it into an async one.
    /// Note that the under the hood this adds an adapter layer which runs
    /// the synchronous connection on a separate thread -- it is not "natively"
    /// async.
    #[maybe_async_cfg::only_if(key = "sync")]
    #[cfg(feature = "async-adapter")]
    pub fn into_async(self) -> Result<ConnectionAsync> {
        Ok(adapter::AsyncAdapter::new(|| Ok(self))?.into_connection())
    }

    /// Runs the provided function with a synchronous wrapper around this asynchronous connection.
    ///
    /// Because this relies on some (safe) memory gymnastics,
    /// there is a small but nonzero risk that if certain tokio calls fail unexpectedly at
    /// the wrong place the the connection will be poisoned -- all subsequent calls
    /// to all methods will fail.
    #[maybe_async_cfg::only_if(key = "async")]
    pub async fn with_sync<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut SyncAdapter<Self>) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let mut conn2 = Connection::new(Box::new(dummy::DummyConnection::new()));
        std::mem::swap(&mut conn2, self);
        let ret: Result<(Result<T>, Connection)> = tokio::task::spawn_blocking(|| {
            let mut sync_conn = SyncAdapter::new(conn2)?;
            let f_ret = f(&mut sync_conn);
            let async_conn = sync_conn.into_inner();
            Ok((f_ret, async_conn))
        })
        .await?;
        match ret {
            Ok((inner_ret, mut conn)) => {
                std::mem::swap(&mut conn, self);
                inner_ret
            }
            // Self is poisoned
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "async")]
impl ConnectionAsync {
    /// Consume this connection and convert it into a synchronous one.
    /// Note that the under the hood this adds an adapter layer which drives
    /// the async connection  -- the async machinery is not eliminated.
    pub fn into_sync(self) -> Result<Connection> {
        Ok(SyncAdapter::new(self)?.into_connection())
    }
}

#[maybe_async_cfg::maybe(
    idents(
        BackendConnection(sync = "BackendConnection"),
        Connection(sync = "Connection"),
        Transaction(sync = "Transaction")
    ),
    sync(keep_self),
    async(feature = "async")
)]
#[async_trait]
impl BackendConnection for Connection {
    async fn transaction(&mut self) -> Result<Transaction> {
        self.conn.transaction().await
    }
    fn backend(&self) -> Box<dyn Backend> {
        self.conn.backend()
    }
    fn backend_name(&self) -> &'static str {
        self.conn.backend_name()
    }
    fn is_closed(&self) -> bool {
        self.conn.is_closed()
    }
}
connection_method_wrapper!(Connection);

#[maybe_async_cfg::maybe(
    idents(ConnectionMethods(sync = "ConnectionMethods"), AsyncRequiresSend),
    sync(keep_self),
    async(feature = "async")
)]
#[async_trait]
pub(super) trait BackendTransaction<'c>:
    ConnectionMethods + internal::AsyncRequiresSend + Debug
{
    /// Commit the transaction.
    ///
    /// Unfortunately because we use this as a trait object, we can't consume self.
    /// It should be understood that no methods should be called after commit.
    /// This trait is not public, and that behavior is enforced by Transaction.
    async fn commit(&mut self) -> Result<()>;
    /// Roll back the transaction. Same comment about consuming self as above.
    async fn rollback(&mut self) -> Result<()>;

    // Workaround for https://github.com/rust-lang/rfcs/issues/2765
    fn connection_methods(&self) -> &dyn ConnectionMethods;
}

/// Database transaction.
///
/// Begin a transaction using the `BackendConnection`
/// [`transaction`][crate::db::BackendConnection::transaction] method.
#[maybe_async_cfg::maybe(
    idents(BackendTransaction(sync = "BackendTransaction")),
    sync(self = "Transaction"),
    async(feature = "async")
)]
#[derive(Debug)]
pub struct Transaction<'c> {
    pub(super) trans: Box<dyn BackendTransaction<'c> + 'c>,
}

#[maybe_async_cfg::maybe(
    idents(
        BackendTransaction(sync = "BackendTransaction"),
        ConnectionMethods(sync = "ConnectionMethods")
    ),
    sync(keep_self),
    async(feature = "async")
)]
impl<'c> Transaction<'c> {
    // unused may occur if no backends are selected
    #[allow(unused)]
    pub(super) fn new(trans: Box<dyn BackendTransaction<'c> + 'c>) -> Self {
        Transaction { trans }
    }
    /// Commit the transaction.
    pub async fn commit(mut self) -> Result<()> {
        self.trans.commit().await
    }
    /// Roll back the transaction. Equivalent to dropping it.
    pub async fn rollback(mut self) -> Result<()> {
        self.trans.deref_mut().rollback().await
    }
    // For use with connection_method_wrapper macro.
    #[allow(clippy::unnecessary_wraps)]
    fn wrapped_connection_methods(&self) -> Result<&dyn ConnectionMethods> {
        let a: &dyn BackendTransaction<'c> = self.trans.as_ref();
        Ok(a.connection_methods())
    }
}

connection_method_wrapper!(Transaction<'_>);

#[maybe_async_cfg::maybe(
    idents(
        BackendTransaction(sync = "BackendTransaction"),
        ConnectionMethods(sync = "ConnectionMethods")
    ),
    sync(keep_self),
    async(feature = "async")
)]
#[async_trait]
impl<'c> BackendTransaction<'c> for Transaction<'c> {
    async fn commit(&mut self) -> Result<()> {
        self.trans.commit().await
    }
    async fn rollback(&mut self) -> Result<()> {
        self.trans.deref_mut().rollback().await
    }
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

#[maybe_async_cfg::maybe(
    idents(
        BackendTransaction(sync = "BackendTransaction"),
        ConnectionMethods(sync = "ConnectionMethods")
    ),
    keep_self,
    sync(),
    async(feature = "async")
)]
#[async_trait]
impl<'c> BackendTransaction<'c> for Box<dyn BackendTransaction<'c> + 'c> {
    async fn commit(&mut self) -> Result<()> {
        self.deref_mut().commit().await
    }
    async fn rollback(&mut self) -> Result<()> {
        self.deref_mut().rollback().await
    }
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

#[maybe_async_cfg::maybe(
    idents(
        BackendTransaction(sync = "BackendTransaction"),
        ConnectionMethods(sync = "ConnectionMethods")
    ),
    keep_self,
    sync(),
    async(feature = "async")
)]
#[async_trait]
impl<'bt> ConnectionMethods for Box<dyn BackendTransaction<'bt> + 'bt> {
    async fn execute(&self, sql: &str) -> Result<()> {
        self.deref().execute(sql).await
    }
    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.deref()
            .query(table, columns, expr, limit, offset, sort)
            .await
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.deref()
            .insert_returning_pk(table, columns, pkcol, values)
            .await
    }
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref().insert_only(table, columns, values).await
    }
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref()
            .insert_or_replace(table, columns, pkcol, values)
            .await
    }
    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref().update(table, pkcol, pk, columns, values).await
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.deref().delete_where(table, expr).await
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        self.deref().has_table(table).await
    }
}

/// Database backend. A boxed implementation can be returned by name via [get_backend][crate::db::get_backend].
#[async_trait]
pub trait Backend: Send + Sync + DynClone {
    fn name(&self) -> &'static str;
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String>;
    /// Establish a new sync connection. The format of the connection
    /// string is backend-dependent.
    fn connect(&self, conn_str: &str) -> Result<Connection>;
    /// Establish a new async connection. The format of the connection
    /// string is backend-dependent.
    #[cfg(feature = "async")]
    async fn connect_async(&self, conn_str: &str) -> Result<ConnectionAsync>;
}

dyn_clone::clone_trait_object!(Backend);

/// Connection specification. Contains the name of a database backend
/// and the backend-specific connection string. See [`connect`]
/// to make a [`Connection`] from a `ConnectionSpec`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConnectionSpec {
    pub backend_name: String,
    pub conn_str: String,
}
impl ConnectionSpec {
    pub fn new(backend_name: impl Into<String>, conn_str: impl Into<String>) -> Self {
        ConnectionSpec {
            backend_name: backend_name.into(),
            conn_str: conn_str.into(),
        }
    }
    /// Save the connection spec to the filesystem for later use.
    pub fn save(&self, path: &Path) -> Result<()> {
        let path = conn_complete_if_dir(path);
        let mut f = fs::File::create(path)?;
        let mut contents = serde_json::to_string_pretty(self)?;
        contents.push('\n');
        f.write_all(contents.as_bytes()).map_err(|e| e.into())
    }
    /// Load a previously saved connection spec
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = conn_complete_if_dir(path.as_ref());
        serde_json::from_reader(fs::File::open(path)?).map_err(|e| e.into())
    }
    pub fn get_backend(&self) -> Result<Box<dyn Backend>> {
        match get_backend(&self.backend_name) {
            Some(backend) => Ok(backend),
            None => Err(crate::Error::UnknownBackend(self.backend_name.clone())),
        }
    }
}

fn conn_complete_if_dir(path: &Path) -> Cow<Path> {
    if path.is_dir() {
        Cow::from(path.join("connection.json"))
    } else {
        Cow::from(path)
    }
}

#[async_trait]
impl Backend for Box<dyn Backend> {
    fn name(&self) -> &'static str {
        self.deref().name()
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String> {
        self.deref().create_migration_sql(current, ops)
    }
    fn connect(&self, conn_str: &str) -> Result<Connection> {
        self.deref().connect(conn_str)
    }
    #[cfg(feature = "async")]
    async fn connect_async(&self, conn_str: &str) -> Result<ConnectionAsync> {
        self.deref().connect_async(conn_str).await
    }
}

/// Find a backend by name.
pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    match name {
        #[cfg(feature = "sqlite")]
        sqlite::BACKEND_NAME => Some(Box::new(sqlite::SQLiteBackend::new())),
        #[cfg(feature = "pg")]
        pg::BACKEND_NAME => Some(Box::new(pg::PgBackend::new())),
        _ => None,
    }
}

/// Connect to a database.
///
/// For non-boxed connections, see individual [`Backend`] implementations.
pub fn connect(spec: &ConnectionSpec) -> Result<Connection> {
    get_backend(&spec.backend_name)
        .ok_or_else(|| Error::UnknownBackend(spec.backend_name.clone()))?
        .connect(&spec.conn_str)
}

/// Connect to a database async.
///
/// For non-boxed connections, see individual [`Backend`] implementations.
#[cfg(feature = "async")]
pub async fn connect_async(spec: &ConnectionSpec) -> Result<ConnectionAsync> {
    get_backend(&spec.backend_name)
        .ok_or_else(|| Error::UnknownBackend(spec.backend_name.clone()))?
        .connect_async(&spec.conn_str)
        .await
}
