use failure;
use failure::Fail;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::cmp::{Eq, PartialEq};
use std::default::Default;

#[cfg(feature = "datetime")]
mod chrono;
pub mod db;
pub mod fkey;
pub mod many;
pub mod migrations;
pub mod query;
pub mod sqlval;

use db::internal::{Column, ConnectionMethods, Row};

pub use query::Query;
pub use sqlval::*;

pub type Result<T> = std::result::Result<T, crate::Error>;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct ObjectState {
    pub saved: bool,
}
impl PartialEq<ObjectState> for ObjectState {
    fn eq(&self, _other: &ObjectState) -> bool {
        // Always return true. This effectively removes ObjectState
        // from participating in equality tests between two objects
        true
    }
}
impl Eq for ObjectState {}

/// A type which may be the result of a database query.
///
/// Every result type must have a corresponding object type and the
/// columns of the result type must be a subset of the columns of the
/// object type. The purpose of a result type which is not also an
/// object type is to allow a query to retrieve a subset of an
/// object's columns.
pub trait DataResult: Sized {
    /// Corresponding object type.
    type DBO: DataObject;
    type Fields: Default;
    const COLUMNS: &'static [Column];
    fn from_row(row: Row) -> Result<Self>
    where
        Self: Sized;
    /// Create a blank query (matching all rows) for this type.
    fn query() -> Query<Self>;
}

/// An object in the database.
///
/// Rather than implementing this type manually, use the
/// `#[model]` attribute.
pub trait DataObject: DataResult<DBO = Self> {
    /// The type of the primary key field.
    type PKType: FieldType + Clone + PartialEq;
    /// The name of the primary key column.
    const PKCOL: &'static str;
    /// The name of the table.
    const TABLE: &'static str;
    /// Get the primary key
    fn pk(&self) -> &Self::PKType;
    /// Find this object in the database based on primary key.
    fn get(conn: &impl ConnectionMethods, id: impl Borrow<Self::PKType>) -> Result<Self>
    where
        Self: Sized,
    {
        <Self as DataResult>::query()
            .filter(query::BoolExpr::Eq(
                Self::PKCOL,
                query::Expr::Val(id.borrow().to_sql()),
            ))
            .limit(1)
            .load(conn)?
            .into_iter()
            .nth(0)
            .ok_or(Error::NoSuchObject.into())
    }
    /// Save the object to the database.
    fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()>;
    /// Delete the object from the database.
    fn delete(&self, conn: &impl ConnectionMethods) -> Result<()>;
}

pub trait ModelTyped {
    type Model: DataObject;
}

/// Propane errors.
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "No such object exists")]
    NoSuchObject,
    #[fail(display = "Index out of bounds {}", 0)]
    BoundsError(String),
    #[fail(
        display = "Type mismatch converting SqlVal. Expected {}, found value {:?}",
        0, 1
    )]
    CannotConvertSqlVal(SqlType, SqlVal),
    #[fail(display = "Mismatch between sql types and rust types while loading data.")]
    SqlResultTypeMismatch,
    #[fail(display = "SqlType not known for {}", ty)]
    UnknownSqlType { ty: String },
    #[fail(display = "Value has not been loaded from the database")]
    ValueNotLoaded,
    #[fail(display = "Not initialized")]
    NotInitialized,
    #[fail(display = "Already initialized")]
    AlreadyInitialized,
    #[fail(display = "Migration error {}", 0)]
    MigrationError(String),
    #[fail(display = "Unknown backend {}", 0)]
    UnknownBackend(String),
    #[fail(display = "Range error")]
    OutOfRange,
    #[fail(display = "Internal logic error")]
    Internal,
    #[fail(display = "Cannot resolve type {}", 0)]
    CannotResolveType(String),
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
            FromSqlError::InvalidType => Error::SqlResultTypeMismatch,
            FromSqlError::OutOfRange(_) => Error::OutOfRange,
            FromSqlError::Other(e2) => Error::Generic(failure::Error::from_boxed_compat(e2)),
        }
    }
}

/// Enumeration of the types a database value may take.
///
/// See also [`SqlVal`][crate::SqlVal].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SqlType {
    Bool,
    /// 4 bytes
    Int,
    /// 8 bytes
    BigInt,
    /// 8 byte float
    Real,
    Text,
    // TODO properly support and test timestamp
    #[cfg(feature = "datetime")]
    Timestamp,
    // TODO properly support and test blob
    Blob,
}
impl std::fmt::Display for SqlType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use SqlType::*;
        match &self {
            Bool => "bool",
            Int => "int",
            BigInt => "big int",
            Real => "float",
            Text => "string",
            #[cfg(feature = "datetime")]
            Timestamp => "timestamp",
            Blob => "blog",
        }
        .fmt(f)
    }
}
