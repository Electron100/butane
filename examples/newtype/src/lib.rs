//! Common helpers for the newtype example CLI.

mod models;

use butane::db::Connection;
use butane::{find, AutoPk, DataObject};
pub use models::{Patch, Record};

/// Create a [Record].
pub fn create_record(conn: &Connection, patch: Patch) -> Record {
    let mut record = Record::new(patch);
    record.save(conn).unwrap();
    record
}

/// Fetch a [Record].
pub fn fetch_record(conn: &Connection, id: &AutoPk<i64>) -> Record {
    find!(Record, id == { id }, conn).unwrap()
}
