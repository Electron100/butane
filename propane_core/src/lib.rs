use failure;
use failure::Fail;
use serde::{Deserialize, Serialize};
use std::default::Default;

pub mod adb;
pub mod db;
pub mod field;
pub mod fkey;
pub mod migrations;
pub mod query;
pub mod sqlval;

pub use adb::*;
pub use query::Query;
pub use sqlval::*;

pub type Result<T> = std::result::Result<T, crate::Error>;

pub trait DBResult: Sized {
    type DBO: DBObject;
    type Fields: Default;
    const COLUMNS: &'static [db::Column];
    fn from_row(row: db::Row) -> Result<Self>
    where
        Self: Sized;
}

pub trait DBObject: DBResult<DBO = Self> {
    type PKType: FieldType + Clone + PartialEq;
    const PKCOL: &'static str;
    const TABLE: &'static str;
    fn pk(&self) -> &Self::PKType;
    fn get(conn: &impl db::BackendConnection, id: Self::PKType) -> Result<Self>
    where
        Self: Sized;
    fn query() -> Query<Self>;
    fn save(&mut self, conn: &impl db::BackendConnection) -> Result<()>;
    fn delete(&self, conn: &impl db::BackendConnection) -> Result<()>;
}

pub trait ModelTyped {
    type Model: DBObject;
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "No such object exists")]
    NoSuchObject,
    #[fail(display = "Index out of bounds")]
    BoundsError,
    #[fail(display = "Type mismatch")]
    TypeMismatch,
    #[fail(display = "SqlType not known for {}", ty)]
    UnknownSqlType { ty: String },
    #[fail(display = "Table {} has no primary key", table)]
    NoPK { table: String },
    #[fail(display = "Value has not been loaded from the database")]
    ValueNotLoaded,
    #[fail(display = "Migration error {}", 0)]
    MigrationError(String),
    #[fail(display = "Unknown backend {}", 0)]
    UnknownBackend(String),
    #[fail(display = "Range error")]
    OutOfRange,
    #[fail(display = "(De)serialization error {}", 0)]
    SerdeJson(serde_json::Error),
    #[fail(display = "IO error {}", 0)]
    IO(std::io::Error),
    #[fail(display = "Sqlite error {}", 0)]
    SQLite(rusqlite::Error),
    #[fail(display = "{}", 0)]
    Generic(failure::Error),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::SerdeJson(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::SQLite(e)
    }
}

impl From<rusqlite::types::FromSqlError> for Error {
    fn from(e: rusqlite::types::FromSqlError) -> Self {
        use rusqlite::types::FromSqlError;
        match e {
            FromSqlError::InvalidType => Error::TypeMismatch,
            FromSqlError::OutOfRange(_) => Error::OutOfRange,
            FromSqlError::Other(e2) => Error::Generic(failure::Error::from_boxed_compat(e2)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SqlType {
    Bool,
    Int,    // 4 bytes
    BigInt, // 8 bytes
    Real,   // 8 byte float
    Text,
    Date,
    Timestamp,
    Blob,
}
