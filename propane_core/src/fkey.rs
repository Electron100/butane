use crate::db::internal::ConnectionMethods;
use crate::*;
use once_cell::unsync::OnceCell;
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
///
pub struct ForeignKey<T>
where
    T: DataObject,
{
    // At least one must be initialized, but both need not be
    val: OnceCell<T>,
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
        self.val.get().ok_or(Error::ValueNotLoaded.into())
    }

    /// Loads the value referred to by this foreign key from the
    /// database if necessary and returns a reference to the value.
    pub fn load(&self, conn: &impl ConnectionMethods) -> Result<&T> {
        self.val.get_or_try_init(|| {
            let pk: SqlVal = self.valpk.get().unwrap().clone();
            T::get(conn, T::PKType::from_sql(pk)?)
        })
    }

    fn new_raw() -> Self {
        ForeignKey {
            val: OnceCell::new(),
            valpk: OnceCell::new(),
        }
    }

    fn ensure_valpk(&self) -> &SqlVal {
        match self.valpk.get() {
            Some(sqlval) => return &sqlval,
            None => match self.val.get() {
                Some(val) => self.valpk.set(val.pk().to_sql()).unwrap(),
                None => panic!("Invalid foreign key state"),
            },
        }
        &self.valpk.get().unwrap()
    }
}

impl<T: DataObject> From<T> for ForeignKey<T> {
    fn from(obj: T) -> Self {
        let ret = Self::new_raw();
        ret.val.set(obj).ok();
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
        // it's cloneable. Then we wouldn't have to ensure the pk
        self.ensure_valpk();
        ForeignKey {
            val: OnceCell::new(),
            valpk: self.valpk.clone(),
        }
    }
}
impl<T: DataObject> PartialEq<ForeignKey<T>> for ForeignKey<T> {
    fn eq(&self, other: &ForeignKey<T>) -> bool {
        self.ensure_valpk().eq(other.ensure_valpk())
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
}
impl<T> IntoSql for ForeignKey<T>
where
    T: DataObject,
{
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
}
impl<T> FromSql for ForeignKey<T>
where
    T: DataObject,
{
    fn from_sql(val: SqlVal) -> Result<Self> {
        Ok(ForeignKey {
            valpk: OnceCell::from(val),
            val: OnceCell::new(),
        })
    }
}
impl<T> PartialEq<T> for ForeignKey<T>
where
    T: DataObject,
{
    fn eq(&self, other: &T) -> bool {
        match self.val.get() {
            Some(t) => return t.pk().eq(other.pk()),
            None => match self.valpk.get() {
                Some(valpk) => valpk.eq(&other.pk().to_sql()),
                None => panic!("Invalid foreign key state"),
            },
        }
    }
}
impl<T> PartialEq<&T> for ForeignKey<T>
where
    T: DataObject,
{
    fn eq(&self, other: &&T) -> bool {
        self.eq(*other)
    }
}
