use crate::{Error::TypeMismatch, Result, SqlType};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A database value.
///
/// For conversion between `SqlVal` and other types, see [`FromSql`], [`IntoSql`], and [`ToSql`].
///
/// [`FromSql`]: crate::FromSql
/// [`IntoSql`]: crate::ToSql
/// [`ToSql`]: crate::ToSql
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlVal {
    Bool(bool),
    Int(i64),
    Real(f64),
    Text(String),
    // TODO properly support and test blob
    Blob(Vec<u8>),
}
impl SqlVal {
    pub fn bool(&self) -> Result<bool> {
        match self {
            SqlVal::Bool(val) => Ok(*val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn integer(&self) -> Result<i64> {
        match self {
            SqlVal::Int(val) => Ok(*val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn real(&self) -> Result<f64> {
        match self {
            SqlVal::Real(val) => Ok(*val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn text<'a>(&'a self) -> Result<&'a str> {
        match self {
            SqlVal::Text(val) => Ok(val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn owned_text(self) -> Result<String> {
        match self {
            SqlVal::Text(val) => Ok(val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn blob<'a>(&'a self) -> Result<&'a [u8]> {
        match self {
            SqlVal::Blob(val) => Ok(val),
            _ => Err(TypeMismatch.into()),
        }
    }
    pub fn owned_blob(self) -> Result<Vec<u8>> {
        match self {
            SqlVal::Blob(val) => Ok(val),
            _ => Err(TypeMismatch.into()),
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
///
pub trait FieldType: ToSql + IntoSql + FromSql {
    const SQLTYPE: SqlType;
}

macro_rules! impl_prim_sql {
    ($prim:ty, $variant:ident, $sqltype:ident) => {
        impl FromSql for $prim {
            fn from_sql(val: SqlVal) -> Result<Self> {
                if let SqlVal::$variant(val) = val {
                    Ok(val as $prim)
                } else {
                    Err(crate::Error::TypeMismatch.into())
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
        }
    };
}

impl_prim_sql!(bool, Bool, Bool);
impl_prim_sql!(i64, Int, BigInt);
impl_prim_sql!(i32, Int, Int);
impl_prim_sql!(u32, Int, BigInt);
impl_prim_sql!(f64, Real, Real);
impl_prim_sql!(f32, Real, Real);
impl_prim_sql!(String, Text, Text);
impl_prim_sql!(Vec<u8>, Blob, Blob);

impl ToSql for &str {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Text(self.to_string())
    }
}

impl fmt::Display for SqlVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use SqlVal::*;
        match &self {
            SqlVal::Bool(val) => val.fmt(f),
            Int(val) => val.fmt(f),
            Real(val) => val.fmt(f),
            Text(val) => val.fmt(f),
            Blob(val) => f.write_str(&hex::encode(val)),
        }
    }
}
