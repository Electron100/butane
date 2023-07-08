use crate::db::Column;
use crate::query::{BoolExpr, Expr};
use crate::{DataObject, Error, FieldType, Result, SqlType, SqlVal, ToSql};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use tokio::sync::OnceCell;

#[cfg(feature = "fake")]
use fake::{Dummy, Faker};

fn default_oc<T>() -> OnceCell<Vec<T>> {
    // Same as impl Default for once_cell::unsync::OnceCell
    OnceCell::new()
}

/// Used to implement a many-to-many relationship between models.
///
/// Creates a new table with columns "owner" and "has" If type T has a
/// many-to-many relationship with U, owner type is T::PKType, has is
/// U::PKType. Table name is T_foo_Many where foo is the name of
/// the Many field
//
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Many<T>
where
    T: DataObject,
{
    item_table: Cow<'static, str>,
    owner: Option<SqlVal>,
    owner_type: SqlType,
    #[serde(skip)]
    new_values: Vec<SqlVal>,
    #[serde(skip)]
    removed_values: Vec<SqlVal>,
    #[serde(skip)]
    #[serde(default = "default_oc")]
    all_values: OnceCell<Vec<T>>,
}
impl<T> Many<T>
where
    T: DataObject,
{
    /// Constructs a new Many. `init` must be called before it can be
    /// loaded or saved (or those methods will return
    /// `Error::NotInitialized`). `init` will automatically be called
    /// when a [`DataObject`] with a `Many` field is loaded or saved.
    ///
    /// [`DataObject`]: super::DataObject
    pub fn new() -> Self {
        Many {
            item_table: Cow::Borrowed("not_initialized"),
            owner: None,
            owner_type: SqlType::Int,
            new_values: Vec::new(),
            removed_values: Vec::new(),
            all_values: OnceCell::new(),
        }
    }

    /// Used by macro-generated code. You do not need to call this directly.
    pub fn ensure_init(&mut self, item_table: &'static str, owner: SqlVal, owner_type: SqlType) {
        if self.owner.is_some() {
            return;
        }
        self.item_table = Cow::Borrowed(item_table);
        self.owner = Some(owner);
        self.owner_type = owner_type;
        self.all_values = OnceCell::new();
    }

    /// Adds a value. Returns Err(ValueNotSaved) if the
    /// provided value uses automatic primary keys and appears
    /// to have an uninitialized one.
    pub fn add(&mut self, new_val: &T) -> Result<()> {
        // Check for uninitialized pk
        match new_val.is_saved() {
            Ok(true) => (), // hooray
            Ok(false) => return Err(Error::ValueNotSaved),
            Err(Error::SaveDeterminationNotSupported) => (), // we don't know, so assume it's OK
            Err(e) => return Err(e),                         // unexpected error
        }

        // all_values is now out of date, so clear it
        self.all_values = OnceCell::new();
        self.new_values.push(new_val.pk().to_sql());
        Ok(())
    }

    /// Removes a value.
    pub fn remove(&mut self, val: &T) {
        // all_values is now out of date, so clear it
        self.all_values = OnceCell::new();
        self.removed_values.push(val.pk().to_sql())
    }

    /// Returns a reference to the value. It must have already been loaded. If not, returns Error::ValueNotLoaded
    pub fn get(&self) -> Result<impl Iterator<Item = &T>> {
        self.all_values
            .get()
            .ok_or(Error::ValueNotLoaded)
            .map(|v| v.iter())
    }

    // todo support save and load for sync too
    /// Used by macro-generated code. You do not need to call this directly.
    pub async fn save(&mut self, conn: &impl crate::ConnectionMethods) -> Result<()> {
        let owner = self.owner.as_ref().ok_or(Error::NotInitialized)?;
        while !self.new_values.is_empty() {
            conn.insert_only(
                &self.item_table,
                &self.columns(),
                &[
                    owner.as_ref(),
                    self.new_values.pop().unwrap().as_ref().clone(),
                ],
            )
            .await?;
        }
        if !self.removed_values.is_empty() {
            conn.delete_where(
                &self.item_table,
                BoolExpr::In("has", std::mem::take(&mut self.removed_values)),
            )
            .await?;
        }
        self.new_values.clear();
        Ok(())
    }

    /// Loads the values referred to by this foreign key from the
    /// database if necessary and returns a reference to them.
    pub async fn load(
        &self,
        conn: &impl crate::ConnectionMethods,
    ) -> Result<impl Iterator<Item = &T>> {
        let vals: Result<&Vec<T>> = self
            .all_values
            .get_or_try_init(|| async {
                //if we don't have an owner then there are no values
                let owner: &SqlVal = match &self.owner {
                    Some(o) => o,
                    None => return Ok(Vec::new()),
                };
                let mut vals = T::query()
                    .filter(BoolExpr::Subquery {
                        col: T::PKCOL,
                        tbl2: self.item_table.clone(),
                        tbl2_col: "has",
                        expr: Box::new(BoolExpr::Eq("owner", Expr::Val(owner.clone()))),
                    })
                    .load(conn)
                    .await?;
                // Now add in the values for things not saved to the db yet
                if !self.new_values.is_empty() {
                    vals.append(
                        &mut T::query()
                            .filter(BoolExpr::In(T::PKCOL, self.new_values.clone()))
                            .load(conn)
                            .await?,
                    );
                }
                Ok(vals)
            })
            .await;
        vals.map(|v| v.iter())
    }

    pub fn columns(&self) -> [Column; 2] {
        [
            Column::new("owner", self.owner_type.clone()),
            Column::new("has", <T::PKType as FieldType>::SQLTYPE),
        ]
    }
}
impl<T: DataObject> PartialEq<Many<T>> for Many<T> {
    fn eq(&self, other: &Many<T>) -> bool {
        (self.owner == other.owner) && (self.item_table == other.item_table)
    }
}
impl<T: DataObject> Eq for Many<T> {}
impl<T: DataObject> Default for Many<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "fake")]
/// Fake data support is currently limited to empty Many relationships.
impl<T: DataObject> Dummy<Faker> for Many<T> {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &Faker, _rng: &mut R) -> Self {
        Self::new()
    }
}
