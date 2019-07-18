use crate::*;
use once_cell::unsync::OnceCell;

pub struct ForeignKey<T>
where
    T: DBObject,
{
    val: OnceCell<T>,
    valpk: OnceCell<SqlVal>,
}
impl<T: DBObject> ForeignKey<T> {
    /// Returns a reference to the value. It must have already been loaded. If not, returns Error::ValueNotLoaded
    pub fn get(&self) -> Result<&T> {
        self.val.get().ok_or(Error::ValueNotLoaded.into())
    }

    /// Loads the value referred to by this foreign key from the
    /// database if necessary and returns a reference to the value.
    pub fn load(&self, conn: &impl db::BackendConnection) -> Result<&T> {
        self.val.get_or_try_init(|| {
            let pk: SqlVal = self.valpk.get().unwrap().clone();
            T::get(conn, T::PKType::from_sql(pk)?)
        })
    }
}

impl<T> ToSql for ForeignKey<T>
where
    T: DBObject,
{
    const SQLTYPE: SqlType = <T as DBObject>::PKType::SQLTYPE;
    fn into_sql(self) -> SqlVal {
        match self.valpk.get() {
            Some(sqlval) => sqlval.clone(),
            None => match self.val.get() {
                Some(val) => val.pk().to_sql(),
                None => panic!("Invalid foreign key state"),
            },
        }
    }
}
impl<T> FromSql for ForeignKey<T>
where
    T: DBObject,
{
    fn from_sql(val: SqlVal) -> Result<Self> {
        Ok(ForeignKey {
            valpk: OnceCell::from(val),
            val: OnceCell::new(),
        })
    }
}
impl<T: DBObject> Clone for ForeignKey<T> {
    fn clone(&self) -> Self {
        // Once specialization lands, it would be nice to clone val if
        // it's cloneable
        ForeignKey {
            val: OnceCell::new(),
            valpk: self.valpk.clone(),
        }
    }
}
