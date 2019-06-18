use crate::{Error::TypeMismatch, Result};
use failure::format_err;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlVal {
    Bool(bool),
    Int(i64),
    Real(f64),
    Text(String),
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

impl From<bool> for SqlVal {
    fn from(val: bool) -> Self {
        SqlVal::Bool(val)
    }
}
impl From<i64> for SqlVal {
    fn from(val: i64) -> Self {
        SqlVal::Int(val)
    }
}
impl From<i32> for SqlVal {
    fn from(val: i32) -> Self {
        SqlVal::Int(val.into())
    }
}
impl From<i16> for SqlVal {
    fn from(val: i16) -> Self {
        SqlVal::Int(val.into())
    }
}
impl From<i8> for SqlVal {
    fn from(val: i8) -> Self {
        SqlVal::Int(val.into())
    }
}
impl From<f64> for SqlVal {
    fn from(val: f64) -> Self {
        SqlVal::Real(val)
    }
}
impl From<f32> for SqlVal {
    fn from(val: f32) -> Self {
        SqlVal::Real(val.into())
    }
}
impl From<String> for SqlVal {
    fn from(val: String) -> Self {
        SqlVal::Text(val)
    }
}

pub trait FromSql {
    fn from_sql(val: SqlVal) -> Result<Self>
    where
        Self: Sized;
}
impl FromSql for bool {
    fn from_sql(val: SqlVal) -> Result<Self> {
        if let SqlVal::Bool(b) = val {
            Ok(b)
        } else {
            Err(format_err!("Type mismatch"))
        }
    }
}
impl FromSql for i64 {
    fn from_sql(val: SqlVal) -> Result<Self> {
        if let SqlVal::Int(i) = val {
            Ok(i)
        } else {
            Err(format_err!("Type mismatch"))
        }
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
