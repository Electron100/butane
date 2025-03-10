//! For working with migrations. If using the butane CLI tool, it is
//! not necessary to use these types directly.

#![allow(missing_docs)]

use std::path::Path;

use async_trait::async_trait;
use fallible_iterator::FallibleIterator;
use nonempty::NonEmpty;

use crate::db::{Backend, BackendConnection, BackendRows, Column, ConnectionMethods};
#[cfg(feature = "async")]
use crate::db::{ConnectionAsync, ConnectionMethodsAsync};
use crate::sqlval::{FromSql, SqlValRef, ToSql};
use crate::{db, query, DataObject, DataResult, Error, PrimaryKeyType, Result, SqlType};

pub mod adb;
use adb::{AColumn, ATable, DeferredSqlType, Operation, TypeIdentifier, ADB};

mod migration;
pub use migration::{Migration, MigrationMut};

mod fs;

mod fsmigrations;
pub use fsmigrations::{FsMigration, FsMigrations};
mod memmigrations;
pub use memmigrations::{MemMigration, MemMigrations};

/// A collection of migrations.
#[allow(async_fn_in_trait)] // We don't expect to need to change the Send bounds of the future.
pub trait Migrations: Clone {
    type M: Migration;

    /// Gets the migration with the given name, if it exists
    fn get_migration(&self, name: &str) -> Option<Self::M>;

    /// Get the most recent migration (other than `current`) or `None` if
    /// no migrations have been created.
    fn latest(&self) -> Option<Self::M>;

    /// Returns migrations since the given migration.
    fn migrations_since(&self, since: &Self::M) -> Result<Vec<Self::M>> {
        let mut last = self.latest();
        let mut accum: Vec<Self::M> = Vec::new();
        while let Some(m) = last {
            if m != *since {
                last = match m.migration_from()? {
                    None => None,
                    Some(name) => self.get_migration(&name),
                };
                accum.push(m);
                continue;
            }

            return Ok(accum.into_iter().rev().collect());
        }
        Err(Error::MigrationError("Migration not in chain".to_string()))
    }

    /// Returns all migrations
    fn all_migrations(&self) -> Result<Vec<Self::M>> {
        let mut last = self.latest();
        let mut accum: Vec<Self::M> = Vec::new();
        while let Some(m) = last {
            last = match m.migration_from()? {
                None => None,
                Some(name) => self.get_migration(&name),
            };
            accum.push(m);
        }
        Ok(accum.into_iter().rev().collect())
    }

    /// Get migrations which have not yet been applied to the database
    fn unapplied_migrations(&self, conn: &impl ConnectionMethods) -> Result<Vec<Self::M>> {
        match self.last_applied_migration(conn)? {
            None => self.all_migrations(),
            Some(m) => self.migrations_since(&m),
        }
    }

    /// Get the last migration that has been applied to the database or None
    /// if no migrations have been applied
    fn last_applied_migration(&self, conn: &impl ConnectionMethods) -> Result<Option<Self::M>> {
        if !conn.has_table(ButaneMigration::TABLE)? {
            return Ok(None);
        }
        let migrations: Vec<ButaneMigration> = conn
            .query(
                ButaneMigration::TABLE,
                ButaneMigration::COLUMNS,
                None,
                None,
                None,
                None,
            )?
            .mapped(ButaneMigration::from_row)
            .collect()?;

        let mut m_opt = self.latest();
        while let Some(m) = m_opt {
            if migrations.contains(&ButaneMigration {
                name: m.name().to_string(),
            }) {
                return Ok(Some(m));
            }
            m_opt = m
                .migration_from()?
                .and_then(|name| self.get_migration(&name))
        }
        Ok(None)
    }

    /// Migrate connection forward.
    fn migrate(&self, connection: &mut impl BackendConnection) -> Result<()> {
        let to_apply = self.unapplied_migrations(connection)?;
        for migration in &to_apply {
            crate::info!("Applying migration {}", migration.name());
            migration.apply(connection)?;
        }
        Ok(())
    }

    #[cfg(feature = "async")]
    /// Migrate connection forward.
    async fn migrate_async(&self, conn: &mut ConnectionAsync) -> Result<()>
    where
        Self: Send + 'static,
    {
        let m2 = self.clone();
        conn.with_sync(move |conn| {
            m2.migrate(conn)?;
            Ok(())
        })
        .await
    }

    /// Remove all applied migrations.
    fn unmigrate(&self, connection: &mut impl BackendConnection) -> Result<()> {
        let mut migration = match self.last_applied_migration(connection)? {
            Some(migration) => migration,
            None => return Ok(()),
        };
        migration.downgrade(connection)?;

        while let Ok(Some(migration_name)) = migration.migration_from() {
            migration = self
                .get_migration(&migration_name)
                .ok_or(Error::MigrationError("Migration not in chain".to_string()))?;
            crate::info!("Rolling back migration {}", migration.name());
            migration.downgrade(connection)?;
        }
        Ok(())
    }

    /// Remove all applied migrations.
    #[cfg(feature = "async")]
    async fn unmigrate_async(&self, conn: &mut ConnectionAsync) -> Result<()>
    where
        Self: Send + 'static,
    {
        let m2 = self.clone();
        conn.with_sync(move |conn| {
            m2.unmigrate(conn)?;
            Ok(())
        })
        .await
    }
}

/// Extension of [`Migrations`] to modify the series of migrations.
pub trait MigrationsMut: Migrations
where
    Self::M: MigrationMut,
{
    /// Construct a new, uninitialized migration. You may want to use
    /// `create_migration` instead, which provides higher-level
    /// functionality.
    fn new_migration(&self, name: &str) -> Self::M;

    /// Adds a migration constructed from `new_migration` into the set
    /// of migrations. Should be called after filling in the migration
    /// details. Unnecessary when using `create_migration`.
    fn add_migration(&mut self, m: Self::M) -> Result<()>;

    /// Deletes all migrations -- deleting them from this object (and
    /// any storage backing it). The database schema is not modified,
    /// nor is any other data removed. Use carefully.
    fn delete_migrations(&mut self) -> Result<()>;

    /// Clears all migrations -- deleting them from this object (and
    /// any storage backing it) and deleting the record of their
    /// existence/application from the database. The database schema
    /// is not modified, nor is any other data removed. Use carefully.
    fn clear_migrations(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
        self.delete_migrations()?;
        conn.delete_where(ButaneMigration::TABLE, query::BoolExpr::True)?;
        Ok(())
    }

    /// Get a pseudo-migration representing the current state as
    /// determined by the last build of models. This does not
    /// necessarily match the current state of the database if
    /// migrations have not yet been applied.
    ///
    /// This migration is named "current". It is not a "real" migration
    /// - it should never be applied
    /// - it will never be returned by `latest`, `migrations_since`, `all_migrations` or other similar methods.
    fn current(&mut self) -> &mut Self::M;

    /// Clears the current state (as would be returned by the `current` method).
    fn clear_current(&mut self) -> Result<()>;

    /// Create a migration `from` -> `current` named `name`. From may be None, in which
    /// case the migration is created from an empty database.
    /// Returns true if a migration was created, false if `from` and `current` represent identical states.
    fn create_migration(
        &mut self,
        backends: &NonEmpty<Box<dyn Backend>>,
        name: &str,
        from: Option<&Self::M>,
    ) -> Result<bool> {
        let to_db = self.current().db()?;
        self.create_migration_to(backends, name, from, to_db)
    }

    /// Create a migration `from` -> `to_db` named `name`. From may be None, in which
    /// case the migration is created from an empty database.
    /// Returns true if a migration was created, false if `from` and `current` represent identical states.
    fn create_migration_to(
        &mut self,
        backends: &NonEmpty<Box<dyn Backend>>,
        name: &str,
        from: Option<&Self::M>,
        to_db: ADB,
    ) -> Result<bool> {
        let empty_db = Ok(ADB::new());
        let from_none = from.is_none();
        let from_db = from.map_or(empty_db, |m| m.db())?;
        let mut ops = adb::diff(&from_db, &to_db);
        if ops.is_empty() {
            return Ok(false);
        }

        let mut modified_tables: Vec<String> = Vec::new();

        for op in &ops {
            match op {
                Operation::AddTable(table)
                | Operation::AddTableConstraints(table)
                | Operation::AddTableIfNotExists(table) => modified_tables.push(table.name.clone()),
                Operation::AddColumn(table_name, _) => modified_tables.push(table_name.clone()),
                Operation::RemoveColumn(table_name, _) => modified_tables.push(table_name.clone()),
                Operation::ChangeColumn(table_name, _, _) => {
                    modified_tables.push(table_name.clone())
                }
                Operation::RemoveTable(_) | Operation::RemoveTableConstraints(_) => {}
            }
        }

        if from_none {
            // This may be the first migration. Create the butane_migration table
            ops.push(Operation::AddTableIfNotExists(migrations_table()));
        }

        let mut m = self.new_migration(name);
        // Save the DB for use by other migrations from this one
        for table in to_db.tables() {
            if modified_tables.contains(&table.name) {
                m.add_modified_table(table)?;
            } else {
                let from =
                    from.ok_or(Error::MigrationError("unmodified requires a from".into()))?;
                m.add_unmodified_table(table, &from.name())?;
            }
        }

        for backend in backends {
            let up_sql = backend.create_migration_sql(&from_db, ops.clone())?;
            let down_sql = backend.create_migration_sql(&to_db, adb::diff(&to_db, &from_db))?;
            m.add_sql(backend.name(), &up_sql, &down_sql)?;
        }

        m.set_migration_from(from.map(|m| m.name().to_string()))?;

        self.add_migration(m)?;
        Ok(true)
    }
}

/// Returns [`ATable`] describing the migration metadata.
pub fn migrations_table() -> ATable {
    let mut table = ATable::new("butane_migrations".to_string());
    let col = AColumn::new(
        "name",
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // references
    );
    table.add_column(col);
    table
}

/// Create a `Migrations` from a filesystem location. The `#[model]`
/// attribute will write migration information to a
/// `butane/migrations` directory under the project directory.
pub fn from_root<P: AsRef<Path>>(path: P) -> FsMigrations {
    FsMigrations::new(path.as_ref().to_path_buf())
}

/// Copies the data in `from` to `to`.
pub fn copy_migration(from: &impl Migration, to: &mut impl MigrationMut) -> Result<()> {
    to.set_migration_from(from.migration_from()?.map(|s| s.to_string()))?;
    let db = from.db()?;
    for table in db.tables() {
        to.add_modified_table(table)?;
    }
    for (k, v) in db.types() {
        to.add_type(k.clone(), v.clone())?;
    }
    for backend_name in from.sql_backends()? {
        let up_sql = from.up_sql(&backend_name)?;
        let down_sql = from.down_sql(&backend_name)?;
        if let (Some(up_sql), Some(down_sql)) = (up_sql, down_sql) {
            to.add_sql(&backend_name, &up_sql, &down_sql)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct ButaneMigration {
    name: String,
}

#[async_trait]
impl DataResult for ButaneMigration {
    type DBO = Self;
    const COLUMNS: &'static [Column] = &[Column::new("name", SqlType::Text)];
    fn from_row(row: &dyn db::BackendRow) -> Result<Self> {
        if row.len() != 1usize {
            return Err(Error::BoundsError(
                "Row has the wrong number of columns for this DataResult".to_string(),
            ));
        }
        Ok(ButaneMigration {
            name: FromSql::from_sql_ref(row.get(0, SqlType::Text).unwrap())?,
        })
    }

    fn query() -> query::Query<Self> {
        query::Query::new("butane_migrations")
    }
}

impl DataObject for ButaneMigration {
    type PKType = String;
    type Fields = (); // we don't need Fields as we never filter
    const PKCOL: &'static str = "name";
    const TABLE: &'static str = "butane_migrations";
    const AUTO_PK: bool = false;
    fn pk(&self) -> &String {
        &self.name
    }
}

impl crate::internal::DataObjectInternal for ButaneMigration {
    const NON_AUTO_COLUMNS: &'static [Column] = Self::COLUMNS;
    fn pk_mut(&mut self) -> &mut impl PrimaryKeyType {
        &mut self.name
    }
    fn non_auto_values(&self, include_pk: bool) -> Vec<SqlValRef> {
        let mut values: Vec<SqlValRef<'_>> = Vec::with_capacity(2usize);
        if include_pk {
            values.push(self.name.to_sql_ref());
        }
        values
    }
    #[cfg(feature = "async")]
    async fn save_many_to_many_async(&mut self, _conn: &impl ConnectionMethodsAsync) -> Result<()> {
        Ok(()) // no-op
    }
    fn save_many_to_many_sync(&mut self, _conn: &impl ConnectionMethods) -> Result<()> {
        Ok(()) // no-op
    }
}
