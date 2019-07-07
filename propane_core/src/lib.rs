use failure;
use failure::Fail;
use serde::{Deserialize, Serialize};

pub mod adb;
pub mod db;
pub mod field;
pub mod migrations;
pub mod query;
pub mod sqlval;

pub use adb::*;
pub use query::Query;
pub use sqlval::*;

pub type Result<T> = std::result::Result<T, failure::Error>;

pub trait DBResult: Sized {
    type DBO;
    const COLUMNS: &'static [db::Column];
    fn from_row(row: db::Row) -> Result<Self>
    where
        Self: Sized;
}

pub trait DBObject: DBResult<DBO = Self> {
    type PKType;
    fn get(conn: &impl db::BackendConnection, id: Self::PKType) -> Result<Self>
    where
        Self: Sized;
    fn query() -> Query<Self>;
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

pub struct ForeignKey<T> {
    val: T,
}
