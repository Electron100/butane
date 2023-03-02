use crate::db::ConnectionMethods;
use crate::*;
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};

/// Used to implement a relationship between models.
///
/// Initialize using `From` or `from_pk`
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
pub struct ForeignKey<T>
where
    T: DataObject,
{
    // At least one must be initialized (enforced internally by this
    // type), but both need not be
    val: OnceCell<Box<T>>,
    valpk: OnceCell<SqlVal>,
}
impl<T: DataObject> ForeignKey<T> {
    pub fn from_pk(pk: T::PKType) -> Self {
        let ret = Self::new_raw();
        ret.valpk.set(pk.into_sql()).unwrap();
        ret
    }
    /// Returns a reference to the value. It must have already been loaded. If not, returns Error::ValueNotLoaded
    pub fn get(&self) -> Result<&T> {
        self.val
            .get()
            .map(|v| v.as_ref())
            .ok_or(Error::ValueNotLoaded)
    }

    /// Returns a reference to the primary key of the value.
    pub fn pk(&self) -> T::PKType {
        match self.val.get() {
            Some(v) => v.pk().clone(),
            None => match self.valpk.get() {
                Some(pk) => T::PKType::from_sql_ref(pk.as_ref()).unwrap(),
                None => panic!("Invalid foreign key state"),
            },
        }
    }

    /// Loads the value referred to by this foreign key from the
    /// database if necessary and returns a reference to it.
    pub fn load(&self, conn: &impl ConnectionMethods) -> Result<&T> {
        self.val
            .get_or_try_init(|| {
                let pk = self.valpk.get().unwrap();
                T::get(conn, &T::PKType::from_sql_ref(pk.as_ref())?).map(Box::new)
            })
            .map(|v| v.as_ref())
    }

    fn new_raw() -> Self {
        ForeignKey {
            val: OnceCell::new(),
            valpk: OnceCell::new(),
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
impl<T: DataObject> Clone for ForeignKey<T> {
    fn clone(&self) -> Self {
        // Once specialization lands, it would be nice to clone val if
        // it's clone-able. Then we wouldn't have to ensure the pk
        self.ensure_valpk();
        ForeignKey {
            val: OnceCell::new(),
            valpk: self.valpk.clone(),
        }
    }
}

impl<T> AsPrimaryKey<T> for ForeignKey<T>
where
    T: DataObject,
{
    fn as_pk(&self) -> Cow<T::PKType> {
        Cow::Owned(self.pk())
    }
}

impl<T: DataObject> Eq for ForeignKey<T> {}
impl<T: DataObject> Debug for ForeignKey<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.ensure_valpk().fmt(f)
    }
}

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
            val: OnceCell::new(),
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
