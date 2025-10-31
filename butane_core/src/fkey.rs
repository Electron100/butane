//! Implementation of foreign key relationships between models.
#![deny(missing_docs)]
use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::OnceLock;

#[cfg(feature = "fake")]
use fake::{Dummy, Faker};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::util::get_or_init_once_lock;
#[cfg(feature = "async")]
use crate::{util::get_or_init_once_lock_async, ConnectionMethodsAsync};
use crate::{
    AsPrimaryKey, ConnectionMethods, DataObject, Error, FieldType, FromSql, Result, SqlType,
    SqlVal, SqlValRef, ToSql,
};

/// Used to implement a relationship between models.
///
/// Initialize using `From` or `from_pk`
///
/// See [`ForeignKeyOpsSync`] and [`ForeignKeyOpsAsync`] for operations requiring a live database connection.
///
/// # Examples
/// ```ignore
/// #[model]
/// struct Blog {
///   ...
/// }
/// #[model]
/// struct Post {
///   blog: ForeignKey<Blog>,
///   ...
/// }
#[derive(Clone, Debug)]
pub struct ForeignKey<T>
where
    T: DataObject,
{
    // At least one must be initialized (enforced internally by this
    // type), but both need not be
    // Using OnceLock instead of OnceCell because of Sync requirements when working with async.
    val: OnceLock<Box<T>>,
    valpk: OnceLock<SqlVal>,
}
impl<T: DataObject> ForeignKey<T> {
    /// Create a value from a reference to the primary key of the value.
    pub fn from_pk(pk: T::PKType) -> Self {
        let ret = Self::new_raw();
        ret.valpk.set(pk.into_sql()).unwrap();
        ret
    }
    /// Return a reference to the value, that must have already been loaded.
    ///
    /// If not already loaded, returns Error::ValueNotLoaded.
    pub fn get(&self) -> Result<&T> {
        self.val
            .get()
            .map(|v| v.as_ref())
            .ok_or(Error::ValueNotLoaded)
    }

    /// Return a reference to the primary key of the value.
    pub fn pk(&self) -> T::PKType {
        match self.val.get() {
            Some(v) => v.pk().clone(),
            None => match self.valpk.get() {
                Some(pk) => T::PKType::from_sql_ref(pk.as_ref()).unwrap(),
                None => panic!("Invalid foreign key state"),
            },
        }
    }

    fn new_raw() -> Self {
        ForeignKey {
            val: OnceLock::new(),
            valpk: OnceLock::new(),
        }
    }

    fn ensure_valpk(&self) -> &SqlVal {
        match self.valpk.get() {
            Some(sqlval) => return sqlval,
            None => match self.val.get() {
                Some(val) => self.valpk.set(val.pk().to_sql()).unwrap(),
                None => panic!("Invalid foreign key state"),
            },
        }
        self.valpk.get().unwrap()
    }
}

/// [`ForeignKey`] operations which require a `Connection`.
#[allow(async_fn_in_trait)] // Not intended to be implemented outside Butane
#[maybe_async_cfg::maybe(
    idents(ConnectionMethods(sync = "ConnectionMethods"),),
    sync(),
    async(feature = "async")
)]
pub trait ForeignKeyOps<T: DataObject> {
    /// Loads the value referred to by this foreign key from the
    /// database if necessary and returns a reference to it.
    async fn load<'a>(&'a self, conn: &impl ConnectionMethods) -> Result<&'a T>
    where
        T: 'a;
}

#[cfg(feature = "async")]
impl<T: DataObject> ForeignKeyOpsAsync<T> for ForeignKey<T> {
    async fn load<'a>(&'a self, conn: &impl ConnectionMethodsAsync) -> Result<&'a T>
    where
        T: 'a,
    {
        use crate::DataObjectOpsAsync;
        get_or_init_once_lock_async(&self.val, || async {
            let pk = self.valpk.get().unwrap();
            T::get(conn, T::PKType::from_sql_ref(pk.as_ref())?)
                .await
                .map(Box::new)
        })
        .await
        .map(|v| v.as_ref())
    }
}

impl<T: DataObject> ForeignKeyOpsSync<T> for ForeignKey<T> {
    fn load<'a>(&'a self, conn: &impl ConnectionMethods) -> Result<&'a T>
    where
        T: 'a,
    {
        use crate::DataObjectOpsSync;
        get_or_init_once_lock(&self.val, || {
            let pk = self.valpk.get().unwrap();
            T::get(conn, T::PKType::from_sql_ref(pk.as_ref())?).map(Box::new)
        })
        .map(|v| v.as_ref())
    }
}

impl<T: DataObject> From<T> for ForeignKey<T> {
    fn from(obj: T) -> Self {
        let ret = Self::new_raw();
        ret.val.set(Box::new(obj)).ok();
        ret
    }
}
impl<T: DataObject> From<&T> for ForeignKey<T> {
    fn from(obj: &T) -> Self {
        Self::from_pk(obj.pk().clone())
    }
}

impl<T> AsPrimaryKey<T> for ForeignKey<T>
where
    T: DataObject,
{
    fn as_pk(&self) -> Cow<'_, T::PKType> {
        Cow::Owned(self.pk())
    }
}

impl<T: DataObject> Eq for ForeignKey<T> {}

impl<T> ToSql for ForeignKey<T>
where
    T: DataObject,
{
    fn to_sql(&self) -> SqlVal {
        self.ensure_valpk().clone()
    }
    fn to_sql_ref(&self) -> SqlValRef<'_> {
        self.ensure_valpk().as_ref()
    }
    fn into_sql(self) -> SqlVal {
        self.ensure_valpk();
        self.valpk.into_inner().unwrap()
    }
}
impl<T> FieldType for ForeignKey<T>
where
    T: DataObject,
{
    const SQLTYPE: SqlType = <T as DataObject>::PKType::SQLTYPE;
    type RefType = <<T as DataObject>::PKType as FieldType>::RefType;
}
impl<T> FromSql for ForeignKey<T>
where
    T: DataObject,
{
    fn from_sql_ref(valref: SqlValRef) -> Result<Self> {
        Ok(ForeignKey {
            valpk: SqlVal::from(valref).into(),
            val: OnceLock::new(),
        })
    }
}
impl<T, U> PartialEq<U> for ForeignKey<T>
where
    U: AsPrimaryKey<T>,
    T: DataObject,
{
    fn eq(&self, other: &U) -> bool {
        match self.val.get() {
            Some(t) => t.pk().eq(&other.as_pk()),
            None => match self.valpk.get() {
                Some(valpk) => valpk.eq(&other.as_pk().to_sql()),
                None => panic!("Invalid foreign key state"),
            },
        }
    }
}

impl<T> Serialize for ForeignKey<T>
where
    T: DataObject,
    T::PKType: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.pk().serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for ForeignKey<T>
where
    T: DataObject,
    T::PKType: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_pk(T::PKType::deserialize(deserializer)?))
    }
}

#[cfg(feature = "fake")]
/// Fake data support is currently limited to empty ForeignKey relationships.
impl<T: DataObject> Dummy<Faker> for ForeignKey<T> {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &Faker, _rng: &mut R) -> Self {
        Self::new_raw()
    }
}
