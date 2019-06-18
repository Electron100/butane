use super::Result;
use super::{Error::BoundsError, Error::ValueAlreadyRetrieved};
use crate::query::BoolExpr;
use crate::{adb, SqlVal};
use failure::format_err;
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::vec::Vec;

mod helper;
mod sqlite;

pub trait BackendConnection: Send + 'static {
    fn execute(&self, sql: &str) -> Result<()>;
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult>;
}

pub struct Column {
    name: &'static str,
    ty: adb::AType,
}
impl Column {
    pub const fn new(name: &'static str, ty: adb::AType) -> Self {
        Column { name, ty }
    }
    pub fn name(&self) -> &str {
        self.name
    }
    pub fn ty(&self) -> adb::AType {
        self.ty
    }
}

pub struct Row {
    vals: Vec<Option<SqlVal>>,
}
impl Row {
    fn new(vals: Vec<SqlVal>) -> Self {
        Row {
            vals: vals.into_iter().map(|v| Some(v)).collect(),
        }
    }
    pub fn get<'a>(&'a self, idx: usize) -> Result<&'a SqlVal> {
        self.vals
            .get(idx)
            .ok_or(failure::Error::from(BoundsError))?
            .as_ref()
            .ok_or(ValueAlreadyRetrieved.into())
    }
    /// Extracts an owned value out of the row. Can only be done once
    /// for each value (subsequent attempts will return ValueAlreadyRetrieved)
    pub fn retrieve(&mut self, idx: usize) -> Result<SqlVal> {
        let mut val: &mut Option<SqlVal> = self
            .vals
            .get_mut(idx)
            .ok_or(failure::Error::from(BoundsError))?;
        if val.is_none() {
            return Err(ValueAlreadyRetrieved.into());
        }
        let mut tmp = None;
        std::mem::swap(val, &mut tmp);
        Ok(tmp.unwrap())
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
    pub fn retrieve_text(&mut self, idx: usize) -> Result<String> {
        self.retrieve(idx)?.owned_text()
    }
    pub fn retrieve_blob(&mut self, idx: usize) -> Result<Vec<u8>> {
        self.retrieve(idx)?.owned_blob()
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
        .ok_or(format_err!("Unknown backend {}", &spec.backend_name))?
        .connect(&spec.conn_str)
}
