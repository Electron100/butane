//! Uuid support

#![deny(missing_docs)]
use uuid::Uuid;

use crate::{
    Error::CannotConvertSqlVal, FieldType, FromSql, PrimaryKeyType, Result, SqlType, SqlVal,
    SqlValRef, ToSql,
};

impl ToSql for Uuid {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Blob(self.as_bytes().to_vec())
    }
    fn to_sql_ref(&self) -> SqlValRef<'_> {
        SqlValRef::Blob(self.as_bytes())
    }
}
impl FromSql for Uuid {
    fn from_sql_ref(valref: SqlValRef) -> Result<Self> {
        match valref {
            SqlValRef::Blob(bytes) => {
                if let Ok(uuid) = Uuid::from_slice(bytes) {
                    return Ok(uuid);
                }
            }
            // Generally we expect uuid to be a blob, but if we get a
            // string we can try to work with it.
            SqlValRef::Text(text) => {
                if let Ok(uuid) = Uuid::parse_str(text) {
                    return Ok(uuid);
                }
            }
            _ => (),
        }
        Err(CannotConvertSqlVal(SqlType::Blob, valref.into()))
    }
    // No point in implementing a `from_sql` method for greater
    // efficiency since to construct the UUID we always end up copying
    // the bytes.
}

impl FieldType for Uuid {
    const SQLTYPE: SqlType = SqlType::Blob;
    type RefType = Self;
}

impl PrimaryKeyType for Uuid {}
