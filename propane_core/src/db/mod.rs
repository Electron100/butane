use super::Error::BoundsError;
use crate::query::BoolExpr;
use crate::{adb, Error, Result, SqlType, SqlVal};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::vec::Vec;

mod helper;
mod sqlite;

pub enum Modification {
    InsertOnly,
}

pub trait BackendConnection: Send + 'static {
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
    fn delete(&self,
        table: &'static str,
        pkcol: &'static str,
        pk: &SqlVal) -> Result<()>;
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
    /*
    /// Extracts an owned value out of the row. Can only be done once
    /// for each value (subsequent attempts will return ValueAlreadyRetrieved)
    pub fn retrieve(&mut self, idx: usize) -> Result<SqlVal> {
        let val: &mut Option<SqlVal> = self
            .vals
            .get_mut(idx)
            .ok_or(failure::Error::from(BoundsError))?;
        if val.is_none() {
            return Err(ValueAlreadyRetrieved.into());
        }
        let mut tmp = None;
        std::mem::swap(val, &mut tmp);
        Ok(tmp.unwrap())
    }*/
    pub fn get_int(&self, idx: usize) -> Result<i64> {
        self.get(idx)?.integer()
    }
    pub fn get_bool(&self, idx: usize) -> Result<bool> {
        self.get(idx)?.bool()
    }
    pub fn get_real(&self, idx: usize) -> Result<f64> {
        self.get(idx)?.real()
    }

    /*pub fn retrieve_text(&mut self, idx: usize) -> Result<String> {
        self.retrieve(idx)?.owned_text()
    }
    pub fn retrieve_blob(&mut self, idx: usize) -> Result<Vec<u8>> {
        self.retrieve(idx)?.owned_blob()
    }*/
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
    fn execute(&self, sql: &str) -> Result<()> {
        self.conn.execute(sql)
    }
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult> {
        self.conn.query(table, columns, expr, limit)
    }
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        self.conn.insert_or_replace(table, columns, values)
    }
    fn delete(&self,
        table: &'static str,
        pkcol: &'static str,
        pk: &SqlVal) -> Result<()> {
        
        self.conn.delete(table, pkcol, pk)
    }
}

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
