use crate::{
    Error::CannotConvertSqlVal, FieldType, FromSql, IntoSql, Result, SqlType, SqlVal, ToSql,
};
use uuid::Uuid;

impl ToSql for Uuid {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Blob(self.as_bytes().to_vec())
    }
}
impl IntoSql for Uuid {
    fn into_sql(self) -> SqlVal {
        SqlVal::Blob(self.as_bytes().to_vec())
    }
}
impl FromSql for Uuid {
    fn from_sql(val: SqlVal) -> Result<Self> {
        match val {
            SqlVal::Blob(ref bytes) => {
                if let Ok(uuid) = Uuid::from_slice(&bytes) {
                    return Ok(uuid);
                }
            }
            // Generally we expect uuid to be a blob, but if we get a
            // string we can try to work with it.
            SqlVal::Text(ref text) => {
                if let Ok(uuid) = Uuid::parse_str(&text) {
                    return Ok(uuid);
                }
            }
            _ => (),
        }
        Err(CannotConvertSqlVal(SqlType::Blob, val))
    }
}

impl FieldType for Uuid {
    const SQLTYPE: SqlType = SqlType::Blob;
    type RefType = Self;
}
