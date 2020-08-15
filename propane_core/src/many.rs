use crate::db::internal::{Column, ConnectionMethods};
use crate::query::{BoolExpr, Expr};
use crate::{DataObject, Error, FieldType, Result, SqlType, SqlVal, ToSql};
use lazycell::LazyCell;

// Many to many item table:
// columns "owner" and "has"
// If type T has a many-to-many relationship with U,
// owner type is T::PKType, has is U::PKType
// table name is T_ManyToMany_foo where foo is the name of the Many field
//
#[derive(Debug)]
pub struct Many<T>
where
    T: DataObject,
{
    item_table: &'static str,
    owner: Option<SqlVal>,
    owner_type: SqlType,
    new_values: Vec<SqlVal>,
    all_values: LazyCell<Vec<T>>,
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
            item_table: "not_initialized",
            owner: None,
            owner_type: SqlType::Int,
            new_values: Vec::new(),
            all_values: LazyCell::new(),
        }
    }

    /// Used by macro-generated code. You do not need to call this directly.
    pub fn ensure_init(&mut self, item_table: &'static str, owner: SqlVal, owner_type: SqlType) {
        if self.owner.is_some() {
            return;
        }
        self.item_table = item_table;
        self.owner = Some(owner);
        self.owner_type = owner_type;
        self.all_values = LazyCell::new();
    }

    /// Adds a value.
    pub fn add(&mut self, new_val: &T) {
        // all_values is now out of date, so clear it
        self.all_values = LazyCell::new();
        self.new_values.push(new_val.pk().to_sql())
    }

    /// Returns a reference to the value. It must have already been loaded. If not, returns Error::ValueNotLoaded
    pub fn get(&self) -> Result<impl Iterator<Item = &T>> {
        self.all_values
            .borrow()
            .ok_or(Error::ValueNotLoaded)
            .map(|v| v.iter())
    }

    /// Used by macro-generated code. You do not need to call this directly.
    pub fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
        let owner = self.owner.as_ref().ok_or(Error::NotInitialized)?;
        while !self.new_values.is_empty() {
            conn.insert_or_replace(
                self.item_table,
                &self.columns(),
                &[owner.clone(), self.new_values.pop().unwrap()],
            )?;
        }
        self.new_values.clear();
        Ok(())
    }

    /// Loads the values referred to by this foreign key from the
    /// database if necessary and returns a reference to the them.
    pub fn load(&self, conn: &impl ConnectionMethods) -> Result<impl Iterator<Item = &T>> {
        let vals: Result<&Vec<T>> = self.all_values.try_borrow_with(|| {
            //if we don't have an owner then there are no values
            let owner: &SqlVal = match &self.owner {
                Some(o) => o,
                None => return Ok(Vec::new()),
            };
            let mut vals = T::query()
                .filter(BoolExpr::Subquery {
                    col: T::PKCOL,
                    tbl2: self.item_table,
                    tbl2_col: "has",
                    expr: Box::new(BoolExpr::Eq("owner", Expr::Val(owner.clone()))),
                })
                .load(conn)?;
            // Now add in the values for things not saved to the db yet
            vals.append(
                &mut T::query()
                    .filter(BoolExpr::In(T::PKCOL, self.new_values.clone()))
                    .load(conn)?,
            );
            Ok(vals)
        });
        vals.map(|v| v.iter())
    }
    pub fn columns(&self) -> [Column; 2] {
        [
            Column::new("owner", self.owner_type),
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
