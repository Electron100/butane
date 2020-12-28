use crate::*;

/// Represents the primary key for a model. Can be used when comparing
/// against a ForeignKey in a filter expression.
pub struct PrimaryKey<T>
where
    T: DataObject,
{
    pk: T::PKType,
}
impl<T> PrimaryKey<T>
where
    T: DataObject,
{
    pub fn new(pk: T::PKType) -> Self {
        PrimaryKey { pk }
    }

    pub fn pk(&self) -> T::PKType {
        self.pk.clone()
    }
}

impl<T> AsRef<T::PKType> for PrimaryKey<T>
where
    T: DataObject,
{
    fn as_ref(&self) -> &T::PKType {
        &self.pk
    }
}

impl<T> ToSql for PrimaryKey<T>
where
    T: DataObject,
{
    fn to_sql(&self) -> SqlVal {
        self.pk.to_sql()
    }
}

/// Borrowed version of [PrimaryKey].
pub struct PrimaryKeyRef<'a, T>
where
    T: DataObject,
{
    pk: &'a T::PKType,
}

impl<'a, T> PrimaryKeyRef<'a, T>
where
    T: DataObject,
{
    pub fn new(pk: &'a T::PKType) -> Self {
        PrimaryKeyRef { pk }
    }

    pub fn pk(&self) -> &T::PKType {
        self.pk
    }
}

impl<'a, T> ToSql for PrimaryKeyRef<'a, T>
where
    T: DataObject,
{
    fn to_sql(&self) -> SqlVal {
        self.pk.to_sql()
    }
}
