use crate::{Error, Result, SqlVal};

#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;

#[cfg(feature = "datetime")]
pub fn timestamp_from_millis(millis: i64) -> Result<SqlVal> {
    let secs = millis / 1000;
    let msecs = millis % 1000;
    let nsecs = msecs * 1000 * 1000;
    match NaiveDateTime::from_timestamp_opt(secs, nsecs as u32) {
        Some(dt) => Ok(SqlVal::Timestamp(dt)),
        None => Err(Error::OutOfRange),
    }
}
