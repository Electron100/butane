use crate::{Error::CannotConvertSqlVal, Result, SqlType};
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;

/// A database value.
///
/// For conversion between `SqlVal` and other types, see [`FromSql`], [`IntoSql`], and [`ToSql`].
///
/// [`FromSql`]: crate::FromSql
/// [`IntoSql`]: crate::IntoSql
/// [`ToSql`]: crate::ToSql
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlVal {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Text(String),
    // TODO properly support and test blob
    Blob(Vec<u8>),
    #[cfg(feature = "datetime")]
    Timestamp(NaiveDateTime),
}
impl SqlVal {
    pub fn bool(&self) -> Result<bool> {
        match self {
            SqlVal::Bool(val) => Ok(*val),
            _ => Err(CannotConvertSqlVal(SqlType::Bool, self.clone())),
        }
    }
    pub fn integer(&self) -> Result<i64> {
        match self {
            SqlVal::Int(val) => Ok(*val),
            _ => Err(CannotConvertSqlVal(SqlType::Int, self.clone())),
        }
    }
    pub fn real(&self) -> Result<f64> {
        match self {
            SqlVal::Real(val) => Ok(*val),
            _ => Err(CannotConvertSqlVal(SqlType::Real, self.clone())),
        }
    }
    pub fn text<'a>(&'a self) -> Result<&'a str> {
        match self {
            SqlVal::Text(val) => Ok(val),
            _ => Err(CannotConvertSqlVal(SqlType::Text, self.clone())),
        }
    }
    pub fn owned_text(self) -> Result<String> {
        match self {
            SqlVal::Text(val) => Ok(val),
            _ => Err(CannotConvertSqlVal(SqlType::Text, self.clone())),
        }
    }
    pub fn blob<'a>(&'a self) -> Result<&'a [u8]> {
        match self {
            SqlVal::Blob(val) => Ok(val),
            _ => Err(CannotConvertSqlVal(SqlType::Blob, self.clone())),
        }
    }
    pub fn owned_blob(self) -> Result<Vec<u8>> {
        match self {
            SqlVal::Blob(val) => Ok(val),
            _ => Err(CannotConvertSqlVal(SqlType::Blob, self.clone())),
        }
    }
}
impl fmt::Display for SqlVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use SqlVal::*;
        match &self {
            SqlVal::Null => f.write_str("NULL"),
            SqlVal::Bool(val) => val.fmt(f),
            Int(val) => val.fmt(f),
            Real(val) => val.fmt(f),
            Text(val) => val.fmt(f),
            Blob(val) => f.write_str(&hex::encode(val)),
            #[cfg(feature = "datetime")]
            Timestamp(val) => val.format("%+").fmt(f),
        }
    }
}

/// Used to convert another type to a `SqlVal`.
///
/// Unlike [`IntoSql`][crate::IntoSql], the value is not consumed.
pub trait ToSql {
    fn to_sql(&self) -> SqlVal;
}

/// Used to convert another type to a `SqlVal`.
///
/// The value is consumed. For a non-consuming trait, see
/// [`ToSql`][crate::ToSql].
pub trait IntoSql {
    fn into_sql(self) -> SqlVal;
}

impl<T> From<T> for SqlVal
where
    T: IntoSql,
{
    fn from(val: T) -> Self {
        val.into_sql()
    }
}

/// Used to convert a `SqlVal` into another type.
///
/// The `SqlVal` is consumed.
pub trait FromSql {
    fn from_sql(val: SqlVal) -> Result<Self>
    where
        Self: Sized;
}

/// Type suitable for being a database column.
pub trait FieldType: ToSql + IntoSql + FromSql {
    const SQLTYPE: SqlType;
    /// Reference type. Used for ergonomics with String (which has
    /// reference type str). For most, it is Self
    type RefType: ?Sized + ToSql;
}

macro_rules! impl_prim_sql {
    ($prim:ty, $variant:ident, $sqltype:ident) => {
        impl_prim_sql! {$prim, $variant, $sqltype, $prim}
    };
    ($prim:ty, $variant:ident, $sqltype:ident, $reftype: ty) => {
        impl FromSql for $prim {
            fn from_sql(val: SqlVal) -> Result<Self> {
                if let SqlVal::$variant(val) = val {
                    Ok(val as $prim)
                } else {
                    Err(crate::Error::CannotConvertSqlVal(
                        SqlType::$sqltype,
                        val.clone(),
                    ))
                }
            }
        }
        impl IntoSql for $prim {
            fn into_sql(self) -> SqlVal {
                SqlVal::$variant(self.into())
            }
        }
        impl ToSql for $prim {
            fn to_sql(&self) -> SqlVal {
                self.clone().into_sql()
            }
        }
        impl FieldType for $prim {
            const SQLTYPE: SqlType = SqlType::$sqltype;
            type RefType = $reftype;
        }
    };
}

impl_prim_sql!(bool, Bool, Bool);
impl_prim_sql!(i64, Int, BigInt);
impl_prim_sql!(i32, Int, Int);
impl_prim_sql!(u32, Int, BigInt);
impl_prim_sql!(u16, Int, Int);
impl_prim_sql!(i16, Int, Int);
impl_prim_sql!(f64, Real, Real);
impl_prim_sql!(f32, Real, Real);
impl_prim_sql!(String, Text, Text, str);
impl_prim_sql!(Vec<u8>, Blob, Blob);
#[cfg(feature = "datetime")]
impl_prim_sql!(NaiveDateTime, Timestamp, Timestamp);

impl ToSql for &str {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Text((*self).to_string())
    }
}
impl ToSql for str {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Text(self.to_string())
    }
}

impl<T> ToSql for Option<T>
where
    T: ToSql,
{
    fn to_sql(&self) -> SqlVal {
        match self {
            None => SqlVal::Null,
            Some(v) => v.to_sql(),
        }
    }
}
impl<T> IntoSql for Option<T>
where
    T: IntoSql,
{
    fn into_sql(self) -> SqlVal {
        match self {
            None => SqlVal::Null,
            Some(v) => v.into_sql(),
        }
    }
}
impl<T> FromSql for Option<T>
where
    T: FromSql,
{
    fn from_sql(val: SqlVal) -> Result<Self> {
        Ok(match val {
            SqlVal::Null => None,
            _ => Some(T::from_sql(val)?),
        })
    }
}
impl<T> FieldType for Option<T>
where
    T: FieldType,
{
    const SQLTYPE: SqlType = T::SQLTYPE;
    type RefType = Self;
}
