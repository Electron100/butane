use failure;
use failure::Fail;

pub mod adb;
pub mod db;
pub mod field;
pub mod migrations;
pub mod query;
mod sqlval;

pub use adb::*;
pub use query::Query;
pub use sqlval::*;

pub type Result<T> = std::result::Result<T, failure::Error>;

pub trait DBObject: Sized {
    type PKType;
    const COLUMNS: &'static [db::Column];
    fn get(conn: &impl db::BackendConnection, id: Self::PKType) -> Result<Self>
    where
        Self: Sized;
    fn query() -> Query;
    fn from_row(row: db::Row) -> Result<Self>
    where
        Self: Sized;
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "No such object exists")]
    NoSuchObject,
    #[fail(display = "Index out of bounds")]
    BoundsError,
    #[fail(display = "Type mismatch")]
    TypeMismatch,
    #[fail(display = "Value already retrieved")]
    ValueAlreadyRetrieved,
}
