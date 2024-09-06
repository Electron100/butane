//! Implementation of many-to-many relationships between models.
#![deny(missing_docs)]
use crate::db::{Column, ConnectionMethods, ConnectionMethodsAsync};
use crate::query::{BoolExpr, Expr, OrderDirection, Query};
use crate::{sqlval::PrimaryKeyType, DataObject, Error, FieldType, Result, SqlType, SqlVal, ToSql};
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
///
/// See [`ManyOpSync`] and [`ManyOpAsync`] for operations requiring a live database connection.
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
        if !new_val.pk().is_valid() {
            return Err(Error::ValueNotSaved);
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

    /// Query the values referred to by this many relationship from the
    /// database if necessary and returns a reference to them.
    fn query(&self) -> Result<Query<T>> {
        let owner: &SqlVal = match &self.owner {
            Some(o) => o,
            None => return Err(Error::NotInitialized),
        };
        Ok(T::query().filter(BoolExpr::Subquery {
            col: T::PKCOL,
            tbl2: self.item_table.clone(),
            tbl2_col: "has",
            expr: Box::new(BoolExpr::Eq("owner", Expr::Val(owner.clone()))),
        }))
    }

    /// Describes the columns of the Many table
    pub fn columns(&self) -> [Column; 2] {
        [
            Column::new("owner", self.owner_type.clone()),
            Column::new("has", <T::PKType as FieldType>::SQLTYPE),
        ]
    }
}

#[maybe_async_cfg::maybe(
    idents(ConnectionMethods(sync, async = "ConnectionMethodsAsync"), QueryOp),
    sync(),
    async()
)]
/// Loads the values referred to by this many relationship from a
/// database query if necessary and returns a reference to them.
async fn load_query_uncached<'a, T>(
    many: &'a Many<T>,
    conn: &impl ConnectionMethods,
    query: Query<T>,
) -> Result<Vec<T>>
where
    T: DataObject + 'a,
{
    use crate::query::QueryOp;
    let mut vals: Vec<T> = query.load(conn).await?;
    // Now add in the values for things not saved to the db yet
    if !many.new_values.is_empty() {
        vals.append(
            &mut T::query()
                .filter(BoolExpr::In(T::PKCOL, many.new_values.clone()))
                .load(conn)
                .await?,
        );
    }
    Ok(vals)
}

/// Loads the values referred to by this many relationship from a
/// database query if necessary and returns a reference to them.
async fn load_query_async<'a, T>(
    many: &'a Many<T>,
    conn: &impl ConnectionMethodsAsync,
    query: Query<T>,
) -> Result<impl Iterator<Item = &'a T>>
where
    T: DataObject + 'a,
{
    many.all_values
        .get_or_try_init(|| load_query_uncached_async(many, conn, query))
        .await
        .map(|v| v.iter())
}

/// Loads the values referred to by this many relationship from a
/// database query if necessary and returns a reference to them.
fn load_query_sync<'a, T>(
    many: &'a Many<T>,
    conn: &impl ConnectionMethods,
    query: Query<T>,
) -> Result<impl Iterator<Item = &'a T>>
where
    T: DataObject + 'a,
{
    crate::sync::get_or_try_init_tokio_once_cell_sync(&many.all_values, || {
        // TODO it would be nice to avoid this clone
        load_query_uncached_sync(many, conn, query.clone())
    })
    .map(|v| v.iter())
}

/// [`Many`] operations which require a `Connection`
#[allow(async_fn_in_trait)] // Not intended to be implemented outside Butane
#[maybe_async_cfg::maybe(
    idents(ConnectionMethods(sync = "ConnectionMethods"),),
    sync(),
    async()
)]
pub trait ManyOp<T: DataObject> {
    /// Used by macro-generated code. You do not need to call this directly.
    async fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()>;

    /// Delete all references from the database, and any unsaved additions.
    async fn delete(&mut self, conn: &impl ConnectionMethods) -> Result<()>;

    /// Loads the values referred to by this many relationship from the
    /// database if necessary and returns a reference to them.
    async fn load<'a>(
        &'a self,
        conn: &impl ConnectionMethods,
    ) -> Result<impl Iterator<Item = &'a T>>
    where
        T: 'a;

    /// Loads and orders the values referred to by this many relationship from a
    /// database if necessary and returns a reference to them.
    async fn load_ordered<'a>(
        &'a self,
        conn: &impl ConnectionMethods,
        order: OrderDirection,
    ) -> Result<impl Iterator<Item = &'a T>>
    where
        T: 'a;
}

#[maybe_async_cfg::maybe(
    idents(
        ConnectionMethods(sync = "ConnectionMethods"),
        ManyOpInternal,
        ManyOp,
        load_query(sync = "load_query_sync", async = "load_query_async"),
    ),
    keep_self,
    sync(),
    async()
)]
impl<T: DataObject> ManyOp<T> for Many<T> {
    async fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
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

    async fn delete(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
        let owner = self.owner.as_ref().ok_or(Error::NotInitialized)?;
        conn.delete_where(
            &self.item_table,
            BoolExpr::Eq("owner", Expr::Val(owner.clone())),
        )
        .await?;
        self.new_values.clear();
        self.removed_values.clear();
        // all_values is now out of date, so clear it
        self.all_values = OnceCell::new();
        Ok(())
    }

    async fn load<'a>(
        &'a self,
        conn: &impl ConnectionMethods,
    ) -> Result<impl Iterator<Item = &'a T>>
    where
        T: 'a,
    {
        let query = self.query();
        // If not initialised then there are no values
        let vals: Result<Vec<&T>> = if query.is_err() {
            Ok(Vec::new())
        } else {
            Ok(load_query(self, conn, query.unwrap()).await?.collect())
        };
        vals.map(|v| v.into_iter())
    }

    async fn load_ordered<'a>(
        &'a self,
        conn: &impl ConnectionMethods,
        order: OrderDirection,
    ) -> Result<impl Iterator<Item = &'a T>>
    where
        T: 'a,
    {
        let query = self.query();
        // If not initialised then there are no values
        let vals: Result<Vec<&T>> = if query.is_err() {
            Ok(Vec::new())
        } else {
            Ok(
                load_query(self, conn, query.unwrap().order(T::PKCOL, order))
                    .await?
                    .collect(),
            )
        };
        vals.map(|v| v.into_iter())
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
