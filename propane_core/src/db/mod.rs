use super::Error::BoundsError;
use crate::query::BoolExpr;
use crate::{adb, Error, Result, SqlType, SqlVal};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::vec::Vec;

mod helper;
mod macros;
mod sqlite;

// Macros are always exported at the root of the crate
use crate::connection_method_wrapper;

pub enum Modification {
    InsertOnly,
}

pub trait ConnectionMethods {
    fn backend_name(&self) -> &'static str;
    fn execute(&self, sql: &str) -> Result<()>;
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult>;
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()>;
    fn delete(&self, table: &'static str, pkcol: &'static str, pk: &SqlVal) -> Result<()>;
    /// Tests if a table exists in the database.
    fn has_table(&self, table: &'static str) -> Result<bool>;
}

pub trait BackendConnection: ConnectionMethods + Send + 'static {
    fn transaction<'c>(&'c mut self) -> Result<Transaction<'c>>;
}

pub struct Column {
    name: &'static str,
    ty: SqlType,
}
impl Column {
    pub const fn new(name: &'static str, ty: SqlType) -> Self {
        Column { name, ty }
    }
    pub fn name(&self) -> &str {
        self.name
    }
    pub fn ty(&self) -> SqlType {
        self.ty
    }
}

pub struct Row {
    vals: Vec<SqlVal>,
}
impl Row {
    fn new(vals: Vec<SqlVal>) -> Self {
        Row { vals }
    }
    pub fn len(&self) -> usize {
        self.vals.len()
    }
    pub fn get<'a>(&'a self, idx: usize) -> Result<&'a SqlVal> {
        self.vals.get(idx).ok_or(BoundsError)
    }
    pub fn get_int(&self, idx: usize) -> Result<i64> {
        self.get(idx)?.integer()
    }
    pub fn get_bool(&self, idx: usize) -> Result<bool> {
        self.get(idx)?.bool()
    }
    pub fn get_real(&self, idx: usize) -> Result<f64> {
        self.get(idx)?.real()
    }
}
impl IntoIterator for Row {
    type Item = SqlVal;
    type IntoIter = std::vec::IntoIter<SqlVal>;
    fn into_iter(self) -> Self::IntoIter {
        self.vals.into_iter()
    }
}

pub type RawQueryResult = Vec<Row>;

pub type QueryResult<T> = Vec<T>;

pub struct Connection {
    conn: Box<BackendConnection>,
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
    pub fn save(&self, path: &Path) -> Result<()> {
        let path = conn_complete_if_dir(path);
        let mut f = fs::File::create(path)?;
        f.write_all(serde_json::to_string(self)?.as_bytes())
            .map_err(|e| e.into())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let path = conn_complete_if_dir(path);
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

pub fn sqlite_backend() -> impl Backend {
    sqlite::SQLiteBackend::new()
}

pub fn get_backend(name: &str) -> Option<Box<Backend>> {
    match name {
        "sqlite" => Some(Box::new(sqlite_backend())),
        _ => None,
    }
}

pub fn connect(spec: &ConnectionSpec) -> Result<Connection> {
    get_backend(&spec.backend_name)
        .ok_or(Error::UnknownBackend(spec.backend_name.clone()))?
        .connect(&spec.conn_str)
}

trait BackendTransaction<'c>: ConnectionMethods {
    /// Commit the transaction Unfortunately because we use this as a
    /// trait object, we can't consume self. It should be understood
    /// that no methods should be called after commit. This trait is
    /// not public, and that behavior is enforced by Transaction
    fn commit(&mut self) -> Result<()>;
}

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
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        self.trans.insert_or_replace(table, columns, values)
    }
    fn delete(&self, table: &'static str, pkcol: &'static str, pk: &SqlVal) -> Result<()> {
        self.trans.delete(table, pkcol, pk)
    }
    fn has_table(&self, table: &'static str) -> Result<bool> {
        self.trans.has_table(table)
    }
}
