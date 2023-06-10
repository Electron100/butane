//! Types, traits, and methods for interacting with a database.
//!
//! The different ways of referring to a database handle may present
//! some initial confusion.
//! * `ConnectionMethods` is a trait containing the methods available on a database connection or a transaction.
//!    Most methods on a [DataObject][crate::DataObject] or a [Query][crate::query::Query] require an
//!    implementation of `ConnectionMethods`.
//! * `BackendConnection` is a trait representing a direct connection to a database backend. It is a superset
//!   of `ConnectionMethods` and also includes the ability to create a transaction.
//! * `Transaction` is a struct representing a database transaction. It implements `ConnectionMethods`.
//! * `Connection` is a convenience struct containing a boxed `BackendConnection`. It cannot do anything other than
//!    what a `BackendConnection` can do, but allows using a single concrete type that is not tied to a particular
//!    database backend. It is returned by the `connect` method.

use crate::query::BoolExpr;
use crate::{migrations::adb, Error, Result, SqlVal, SqlValRef};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;

mod connmethods;
mod helper;
mod macros;
#[cfg(feature = "pg")]
pub mod pg;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "r2d2")]
pub mod r2;
#[cfg(feature = "r2d2")]
pub use r2::ConnectionManager;

// Macros are always exported at the root of the crate
use crate::connection_method_wrapper;

pub use connmethods::{
    BackendRow, BackendRows, Column, ConnectionMethods, MapDeref, QueryResult, RawQueryResult,
};

/// Database connection.
pub trait BackendConnection: ConnectionMethods + Debug + Send + 'static {
    /// Begin a database transaction. The transaction object must be
    /// used in place of this connection until it is committed and aborted.
    fn transaction(&mut self) -> Result<Transaction>;
    /// Retrieve the backend backend this connection
    fn backend(&self) -> Box<dyn Backend>;
    fn backend_name(&self) -> &'static str;
    /// Tests if the connection has been closed. Backends which do not
    /// support this check should return false.
    fn is_closed(&self) -> bool;
}

/// Database connection. May be a connection to any type of database
/// as it is a boxed abstraction over a specific connection.
#[derive(Debug)]
pub struct Connection {
    conn: Box<dyn BackendConnection>,
}
impl Connection {
    pub fn execute(&mut self, sql: impl AsRef<str>) -> Result<()> {
        self.conn.execute(sql.as_ref())
    }
    // For use with connection_method_wrapper macro
    #[allow(clippy::unnecessary_wraps)]
    fn wrapped_connection_methods(&self) -> Result<&dyn BackendConnection> {
        Ok(self.conn.as_ref())
    }
}
impl BackendConnection for Connection {
    fn transaction(&mut self) -> Result<Transaction> {
        self.conn.transaction()
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

/// Connection specification. Contains the name of a database backend
/// and the backend-specific connection string. See [connect][crate::db::connect]
/// to make a [Connection][crate::db::Connection] from a `ConnectionSpec`.
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
        f.write_all(serde_json::to_string(self)?.as_bytes())
            .map_err(|e| e.into())
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

/// Database backend. A boxed implementation can be returned by name via [get_backend][crate::db::get_backend].
pub trait Backend {
    fn name(&self) -> &'static str;
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String>;
    fn connect(&self, conn_str: &str) -> Result<Connection>;
}

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

/// Connect to a database. For non-boxed connections, see individual
/// [Backend][crate::db::Backend] implementations.
pub fn connect(spec: &ConnectionSpec) -> Result<Connection> {
    get_backend(&spec.backend_name)
        .ok_or_else(|| Error::UnknownBackend(spec.backend_name.clone()))?
        .connect(&spec.conn_str)
}

trait BackendTransaction<'c>: ConnectionMethods + Debug {
    /// Commit the transaction Unfortunately because we use this as a
    /// trait object, we can't consume self. It should be understood
    /// that no methods should be called after commit. This trait is
    /// not public, and that behavior is enforced by Transaction
    fn commit(&mut self) -> Result<()>;
    /// Roll back the transaction. Same comment about consuming self as above.
    fn rollback(&mut self) -> Result<()>;

    // Workaround for https://github.com/rust-lang/rfcs/issues/2765
    fn connection_methods(&self) -> &dyn ConnectionMethods;
    fn connection_methods_mut(&mut self) -> &mut dyn ConnectionMethods;
}

/// Database transaction.
///
/// Begin a transaction using the `BackendConnection`
/// [`transaction`][crate::db::BackendConnection::transaction] method.
#[derive(Debug)]
pub struct Transaction<'c> {
    trans: Box<dyn BackendTransaction<'c> + 'c>,
}
impl<'c> Transaction<'c> {
    // unused may occur if no backends are selected
    #[allow(unused)]
    fn new(trans: Box<dyn BackendTransaction<'c> + 'c>) -> Self {
        Transaction { trans }
    }
    /// Commit the transaction
    pub fn commit(mut self) -> Result<()> {
        self.trans.deref_mut().commit()
    }
    /// Roll back the transaction. Equivalent to dropping it.
    pub fn rollback(mut self) -> Result<()> {
        self.trans.deref_mut().rollback()
    }
    // For use with connection_method_wrapper macro
    #[allow(clippy::unnecessary_wraps)]
    fn wrapped_connection_methods(&self) -> Result<&dyn ConnectionMethods> {
        let a: &dyn BackendTransaction<'c> = self.trans.as_ref();
        Ok(a.connection_methods())
    }
}

connection_method_wrapper!(Transaction<'_>);
