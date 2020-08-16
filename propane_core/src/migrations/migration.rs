use super::adb::{ATable, DeferredSqlType, TypeKey, ADB};
use super::PropaneMigration;
use crate::db::ConnectionMethods;
use crate::{db, sqlval::ToSql, DataObject, DataResult, Error, Result};
use std::borrow::Cow;
use std::cmp::PartialEq;

/// Type representing a database migration. A migration describes how
/// to bring the database from state A to state B. In general, the
/// methods on this type are persistent -- they read from and write to
/// the filesystem.
///
/// A Migration cannot be constructed directly, only retrieved from
/// [Migrations][crate::migrations::Migrations].
pub trait Migration: PartialEq {
    /// Retrieves the full abstract database state describing all tables
    fn db(&self) -> Result<ADB>;

    /// Get the name of the migration before this one (if any).
    fn migration_from(&self) -> Result<Option<Cow<str>>>
    where
        Self: Sized;

    /// The name of this migration.
    fn name(&self) -> Cow<str>;

    /// The backend-specific commands to apply this migration.
    fn up_sql(&self, backend_name: &str) -> Result<Option<String>>;

    /// The backend-specific commands to undo this migration.
    fn down_sql(&self, backend_name: &str) -> Result<Option<String>>;

    /// The names of the backends this migration has sql for.
    fn sql_backends(&self) -> Result<Vec<String>>;

    /// Apply the migration to a database connection. The connection
    /// must be for the same type of database as this and the database
    /// must be in the state of the migration prior to this one
    fn apply(&self, conn: &mut impl db::BackendConnection) -> Result<()> {
        let tx = conn.transaction()?;
        let sql = self
            .up_sql(tx.backend_name())?
            .ok_or_else(|| Error::UnknownBackend(tx.backend_name().to_string()))?;
        tx.execute(&sql)?;
        tx.insert_or_replace(
            PropaneMigration::TABLE,
            PropaneMigration::COLUMNS,
            &[self.name().as_ref().to_sql()],
        )?;
        tx.commit()
    }
}

/// A migration which can be modified
pub trait MigrationMut: Migration {
    /// Adds an abstract table to the migration. The table state should
    /// represent the expected state after the migration has been
    /// applied. It is expected that all tables will be added to the
    /// migration in this fashion.
    fn write_table(&mut self, table: &ATable) -> Result<()>;

    /// Set the backend-specific commands to apply/undo this migration.
    fn add_sql(&mut self, backend_name: &str, up_sql: &str, down_sql: &str) -> Result<()>;

    /// Adds a TypeKey -> SqlType mapping. Only meaningful on the special current migration.
    fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()>;

    /// Set the name of the migration before this one.
    fn set_migration_from(&mut self, prev: Option<String>) -> Result<()>;
}
