//! Types, traits, and methods for interacting with a database.

use crate::query::BoolExpr;
use crate::{migrations::adb, Error, Result, SqlVal};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::vec::Vec;

mod helper;
pub mod internal;
mod macros;
pub mod sqlite;

// Macros are always exported at the root of the crate
use crate::connection_method_wrapper;

use internal::*;

/// Database connection.
pub trait BackendConnection: ConnectionMethods + Send + 'static {
    /// Begin a database transaction. The transaction object must be
    /// used in place of this connection until it is committed and aborted.
    fn transaction(&mut self) -> Result<Transaction>;
}

/// Database connection. May be a connection to any type of database
/// as it is a boxed abstraction over a specific connection.
pub struct Connection {
    conn: Box<dyn BackendConnection>,
}
impl Connection {
    pub fn execute(&self, sql: impl AsRef<str>) -> Result<()> {
        self.conn.execute(sql.as_ref())
    }
}
impl BackendConnection for Connection {
    fn transaction(&mut self) -> Result<Transaction> {
        self.conn.transaction()
    }
}
connection_method_wrapper!(Connection);

/// Connection specification. Contains the name of a database backend
/// and the backend-specific connection string. See [connect][crate::db::connect]
/// to make a [Connection][crate::db::Connection] from a `ConnectionSpec`.
#[derive(Serialize, Deserialize)]
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
        f.write_all(serde_json::to_string(self)?.as_bytes())
            .map_err(|e| e.into())
    }
    /// Load a previously saved connection spec
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = conn_complete_if_dir(path.as_ref());
        serde_json::from_reader(fs::File::open(path)?).map_err(|e| e.into())
    }
}

fn conn_complete_if_dir(path: &Path) -> Cow<Path> {
    if path.is_dir() {
        Cow::from(path.join("connection.json"))
    } else {
        Cow::from(path)
    }
}

/// Database backend. A boxed implementation can be returned by name via [get_backend][crate::db::get_backend].
pub trait Backend {
    fn get_name(&self) -> &'static str;
    fn create_migration_sql(&self, current: &adb::ADB, ops: &[adb::Operation]) -> String;
    fn connect(&self, conn_str: &str) -> Result<Connection>;
}

impl Backend for Box<dyn Backend> {
    fn get_name(&self) -> &'static str {
        self.deref().get_name()
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: &[adb::Operation]) -> String {
        self.deref().create_migration_sql(current, ops)
    }
    fn connect(&self, conn_str: &str) -> Result<Connection> {
        self.deref().connect(conn_str)
    }
}

/// Find a backend by name.
pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    match name {
        "sqlite" => Some(Box::new(sqlite::SQLiteBackend::new())),
        _ => None,
    }
}

/// Connect to a database. For non-boxed connections, see individual
/// [Backend][crate::db::Backend] implementations.
pub fn connect(spec: &ConnectionSpec) -> Result<Connection> {
    get_backend(&spec.backend_name)
        .ok_or_else(|| Error::UnknownBackend(spec.backend_name.clone()))?
        .connect(&spec.conn_str)
}

trait BackendTransaction<'c>: ConnectionMethods {
    /// Commit the transaction Unfortunately because we use this as a
    /// trait object, we can't consume self. It should be understood
    /// that no methods should be called after commit. This trait is
    /// not public, and that behavior is enforced by Transaction
    fn commit(&mut self) -> Result<()>;
}

/// Database transaction.
///
/// Begin a transaction using the `BackendConnection`
/// [`transaction`][crate::db::BackendConnection::transaction] method.
pub struct Transaction<'c> {
    trans: Box<dyn BackendTransaction<'c> + 'c>,
}
impl<'c> Transaction<'c> {
    fn new(trans: Box<dyn BackendTransaction<'c> + 'c>) -> Self {
        Transaction { trans }
    }
    /// Commit the transaction
    pub fn commit(mut self) -> Result<()> {
        self.trans.deref_mut().commit()
    }
    // TODO need rollback method
}
impl ConnectionMethods for Transaction<'_> {
    fn backend_name(&self) -> &'static str {
        self.trans.backend_name()
    }
    fn execute(&self, sql: &str) -> Result<()> {
        self.trans.execute(sql)
    }
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult> {
        self.trans.query(table, columns, expr, limit)
    }
    fn insert(
        &self,
        table: &'static str,
        columns: &[Column],
        pkcol: Column,
        values: &[SqlVal],
    ) -> Result<SqlVal> {
        self.trans.insert(table, columns, pkcol, values)
    }
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        self.trans.insert_or_replace(table, columns, values)
    }
    fn update(
        &self,
        table: &'static str,
        pkcol: Column,
        pk: SqlVal,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        self.trans.update(table, pkcol, pk, columns, values)
    }
    fn delete_where(&self, table: &'static str, expr: BoolExpr) -> Result<()> {
        self.trans.delete_where(table, expr)
    }
    fn has_table(&self, table: &'static str) -> Result<bool> {
        self.trans.has_table(table)
    }
}
