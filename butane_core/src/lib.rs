//! Library providing functionality used by butane macros and tools.
#![deny(missing_docs)]
#![allow(clippy::iter_nth_zero)]
#![allow(clippy::upper_case_acronyms)] //grandfathered, not going to break API to rename
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::cmp::{Eq, PartialEq};

use thiserror::Error as ThisError;

pub mod codegen;
pub mod custom;
pub mod db;
pub mod fkey;
pub mod many;
pub mod migrations;
pub mod query;
pub mod sqlval;

#[cfg(feature = "uuid")]
pub mod uuid;

mod autopk;
pub use autopk::AutoPk;
use custom::SqlTypeCustom;
use db::{BackendRow, Column, ConnectionMethods};
pub use query::Query;
pub use sqlval::{AsPrimaryKey, FieldType, FromSql, PrimaryKeyType, SqlVal, SqlValRef, ToSql};

/// Result type that uses [`crate::Error`].
pub type Result<T> = std::result::Result<T, crate::Error>;

/// A type which may be the result of a database query.
///
/// Every result type must have a corresponding object type and the
/// columns of the result type must be a subset of the columns of the
/// object type. The purpose of a result type which is not also an
/// object type is to allow a query to retrieve a subset of an
/// object's columns.
#[async_trait]
pub trait DataResult: Sized {
    /// Corresponding object type.
    type DBO: DataObject;

    /// Metadata for eaCH column.
    const COLUMNS: &'static [Column];

    /// Load an object from a database backend row.
    fn from_row<'a>(row: &(dyn BackendRow + 'a)) -> Result<Self>
    where
        Self: Sized;

    /// Create a blank query (matching all rows) for this type.
    fn query() -> Query<Self>;
}

/// An object in the database.
///
/// Rather than implementing this type manually, use the
/// `#[model]` attribute.
#[async_trait(?Send)]
pub trait DataObject: DataResult<DBO = Self> + Sync {
    /// The type of the primary key field.
    type PKType: PrimaryKeyType;
    /// Link to a generated struct providing query helpers for each field.
    type Fields: Default;
    /// The name of the primary key column.
    const PKCOL: &'static str;
    /// The name of the table.
    const TABLE: &'static str;
    /// Whether or not this model uses an automatic primary key set on
    /// the first save.
    const AUTO_PK: bool;

    /// Get the primary key
    fn pk(&self) -> &Self::PKType;
    /// Find this object in the database based on primary key.
    /// Returns `Error::NoSuchObject` if the primary key does not exist.
    async fn get(conn: &impl ConnectionMethods, id: impl ToSql) -> Result<Self>
    where
        Self: Sized,
        Self::PKType: Sync,
    {
        Self::try_get(conn, id).await?.ok_or(Error::NoSuchObject)
    }
    /// Find this object in the database based on primary key.
    /// Returns `None` if the primary key does not exist.
    async fn try_get(conn: &impl ConnectionMethods, id: impl ToSql) -> Result<Option<Self>>
    where
        Self: Sized,
    {
        // todo make sync and async variants
        use crate::query::QueryOpAsync;
        Ok(<Self as DataResult>::query()
            .filter(query::BoolExpr::Eq(
                Self::PKCOL,
                query::Expr::Val(id.borrow().to_sql()),
            ))
            .limit(1)
            .load(conn)
            .await?
            .into_iter()
            .nth(0))
    }

    /// Save the object to the database.
    async fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()>;
    /// Delete the object from the database.
    async fn delete(&self, conn: &impl ConnectionMethods) -> Result<()> {
        conn.delete(Self::TABLE, Self::PKCOL, self.pk().to_sql())
            .await
    }
}

/// ASYNC TODO is this still necessary
pub trait ModelTyped {
    /// ASYNC TODO
    type Model: DataObject;
}

/// Butane errors.
#[allow(missing_docs)]
#[derive(Debug, ThisError)]
pub enum Error {
    #[error("No such object exists")]
    NoSuchObject,
    #[error("Index out of bounds {0}")]
    BoundsError(String),
    #[error("Type mismatch converting SqlVal. Expected {0}, found value {1:?}")]
    CannotConvertSqlVal(SqlType, SqlVal),
    #[error(
        "Mismatch between sql types and rust types while loading data for column {col}. {detail}"
    )]
    SqlResultTypeMismatch { col: String, detail: String },
    #[error("SqlType not known for {0}")]
    UnknownSqlType(String),
    #[error("Value has not been loaded from the database")]
    ValueNotLoaded,
    #[error("Cannot use value not saved to the database")]
    ValueNotSaved,
    #[error("Not initialized")]
    NotInitialized,
    #[error("Already initialized")]
    AlreadyInitialized,
    #[error("Migration error {0}")]
    MigrationError(String),
    #[error("Unknown backend {0}")]
    UnknownBackend(String),
    #[error("Range error")]
    OutOfRange,
    #[error("Internal logic error {0}")]
    Internal(String),
    #[error("Cannot resolve type {0}. Are you missing a #[butane_type] attribute?")]
    CannotResolveType(String),
    #[error("Auto fields are only supported for integer fields. {0} cannot be auto.")]
    InvalidAuto(String),
    #[error("No implicit default available for custom sql types.")]
    NoCustomDefault,
    #[error("No enum variant named '{0}'")]
    UnknownEnumVariant(String),
    #[error("Backend {1} is not compatible with custom SqlVal {0:?}")]
    IncompatibleCustom(custom::SqlValCustom, &'static str),
    #[error("Backend {1} is not compatible with custom SqlType {0:?}")]
    IncompatibleCustomT(custom::SqlTypeCustom, &'static str),
    #[error("Literal values for custom types are currently unsupported.")]
    LiteralForCustomUnsupported(custom::SqlValCustom),
    #[error("This DataObject doesn't support determining whether it has been saved.")]
    SaveDeterminationNotSupported,
    #[error("(De)serialization error {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("IO error {0}")]
    IO(#[from] std::io::Error),
    #[cfg(feature = "sqlite")]
    #[error("Sqlite error {0}")]
    SQLite(#[from] rusqlite::Error),
    #[cfg(feature = "sqlite")]
    #[error("Sqlite error {0}")]
    SQLiteFromSQL(rusqlite::types::FromSqlError),
    #[cfg(feature = "pg")]
    #[error("Postgres error {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[cfg(feature = "datetime")]
    #[error("Chrono error {0}")]
    Chrono(#[from] chrono::ParseError),
    #[error("RefCell error {0}")]
    CellBorrow(#[from] std::cell::BorrowMutError),
    #[cfg(feature = "tls")]
    #[error("TLS error {0}")]
    TLS(#[from] native_tls::Error),
    #[error("Generic error {0}")]
    Generic(#[from] Box<dyn std::error::Error + Sync + Send>),
    #[error("Tokio join error {0}")]
    TokioJoin(#[from] tokio::task::JoinError),
    #[error("Tokio recv error {0}")]
    TokioRecv(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Crossbeam cannot send/recv, channel disconnected")]
    CrossbeamChannel,
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::types::FromSqlError> for Error {
    fn from(e: rusqlite::types::FromSqlError) -> Self {
        use rusqlite::types::FromSqlError;
        match &e {
            FromSqlError::InvalidType => Error::SqlResultTypeMismatch {
                col: "unknown".to_string(),
                detail: "unknown".to_string(),
            },
            FromSqlError::OutOfRange(_) => Error::OutOfRange,
            FromSqlError::Other(_) => Error::SQLiteFromSQL(e),
            _ => Error::SQLiteFromSQL(e),
        }
    }
}

impl<T> From<crossbeam_channel::SendError<T>> for Error {
    fn from(_e: crossbeam_channel::SendError<T>) -> Self {
        Self::CrossbeamChannel
    }
}

impl From<crossbeam_channel::RecvError> for Error {
    fn from(_e: crossbeam_channel::RecvError) -> Self {
        Self::CrossbeamChannel
    }
}

/// Enumeration of the types a database value may take.
///
/// See also [`SqlVal`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SqlType {
    /// Boolean
    Bool,
    /// 4 bytes
    Int,
    /// 8 bytes
    BigInt,
    /// 8 byte float
    Real,
    /// String
    Text,
    #[cfg(feature = "datetime")]
    /// Timestamp
    Timestamp,
    /// Blob
    Blob,
    #[cfg(feature = "json")]
    /// JSON
    Json,
    /// Custom SQL type
    Custom(SqlTypeCustom),
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
            Blob => "blob",
            #[cfg(feature = "json")]
            Json => "json",
            Custom(_) => "custom",
        }
        .fmt(f)
    }
}

#[cfg(feature = "log")]
pub use log::debug;
#[cfg(feature = "log")]
pub use log::warn;

#[cfg(not(feature = "log"))]
mod btlog {
    // this module is just for grouping -- macro_export puts them in the crate root

    /// Noop for when feature log is not enabled.
    #[macro_export]
    macro_rules! debug {
        (target: $target:expr, $($arg:tt)+) => {};
        ($($arg:tt)+) => {};
    }

    /// Noop for when feature log is not enabled.
    #[macro_export]
    macro_rules! warn {
        (target: $target:expr, $($arg:tt)+) => {};
        ($($arg:tt)+) => {};
    }
}
