use super::Result;
use crate::adb;
use failure::format_err;
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::Path;

mod sqlite;

pub trait Connection: Send + 'static {}

#[derive(Serialize, Deserialize)]
pub struct ConnectionSpec {
    backend_name: String,
    conn_str: String,
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
    fn connect_box(&self, conn_str: &str) -> Result<Box<Connection>>;
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

pub fn connect(spec: &ConnectionSpec) -> Result<Box<Connection>> {
    get_backend(&spec.backend_name)
        .ok_or(format_err!("Unknown backend {}", &spec.backend_name))?
        .connect_box(&spec.conn_str)
}
