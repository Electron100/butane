use super::{FieldType, FromSql, PrimaryKeyType, Result, SqlType, SqlVal, SqlValRef, ToSql};
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, PartialOrd};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AutoPk<T: PrimaryKeyType> {
    inner: Option<T>,
}

impl<T: PrimaryKeyType> AutoPk<T> {
    pub fn uninitialized() -> Self
    where
        T: Default,
    {
        Self::default()
    }

    fn with_value(val: T) -> Self {
        AutoPk { inner: Some(val) }
    }
}

impl<T: PrimaryKeyType> FromSql for AutoPk<T> {
    /// Used to convert a SqlValRef into another type.
    fn from_sql_ref(val: SqlValRef<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(AutoPk::with_value(T::from_sql_ref(val)?))
    }

    /// Used to convert a SqlVal into another type. The default
    /// implementation calls `Self::from_sql_ref(val.as_ref())`, which
    /// may be inefficient. This method is chiefly used only for
    /// primary keys: a more efficient implementation is unlikely to
    /// provide benefits for types not used as primary keys.
    fn from_sql(val: SqlVal) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(AutoPk::with_value(T::from_sql(val)?))
    }
}

impl<T: PrimaryKeyType> ToSql for AutoPk<T> {
    fn to_sql(&self) -> SqlVal {
        self.inner
            .as_ref()
            .expect("PK is not generated yet!")
            .to_sql()
    }
    fn to_sql_ref(&self) -> SqlValRef<'_> {
        self.inner
            .as_ref()
            .expect("PK is not generated yet!")
            .to_sql_ref()
    }
    fn into_sql(self) -> SqlVal {
        self.inner.expect("PK is not generated yet!").into_sql()
    }
}

impl<T: PrimaryKeyType> PartialEq for AutoPk<T> {
    fn eq(&self, other: &AutoPk<T>) -> bool {
        if !self.is_valid() || !other.is_valid() {
            false
        } else {
            self.inner.eq(&other.inner)
        }
    }
}

impl<T: PrimaryKeyType> FieldType for AutoPk<T> {
    const SQLTYPE: SqlType = T::SQLTYPE;
    /// Reference type. Used for ergonomics with String (which has
    /// reference type str). For most, it is Self
    type RefType = T::RefType;
}
impl<T: PrimaryKeyType> PrimaryKeyType for AutoPk<T> {
    fn is_valid(&self) -> bool {
        match &self.inner {
            Some(val) => val.is_valid(),
            None => false,
        }
    }
}

impl<T: PrimaryKeyType + std::fmt::Display> std::fmt::Display for AutoPk<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match &self.inner {
            Some(val) => val.fmt(f),
            None => write!(f, "null"),
        }
    }
}

impl<T: PrimaryKeyType + Copy> Copy for AutoPk<T> {}

impl<T: PrimaryKeyType + Ord> PartialOrd for AutoPk<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match &self.inner {
            Some(val) => match &other.inner {
                Some(val2) => val.partial_cmp(val2),
                None => None,
            },
            None => None,
        }
    }
}
