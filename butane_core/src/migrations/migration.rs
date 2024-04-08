use std::borrow::Cow;
use std::fmt::Debug;

use super::adb::{ATable, DeferredSqlType, TypeKey, ADB};
use super::ButaneMigration;
use crate::db::ConnectionMethods;
use crate::query::{BoolExpr, Expr};
use crate::{db, sqlval::ToSql, DataObject, DataResult, Error, Result};

/// Type representing a database migration. A migration describes how
/// to bring the database from state A to state B. In general, the
/// methods on this type are persistent -- they read from and write to
/// the filesystem.
///
/// A Migration cannot be constructed directly, only retrieved from
/// [Migrations][crate::migrations::Migrations].
#[async_trait::async_trait(?Send)]
pub trait Migration: Debug + PartialEq {
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
    async fn apply(&self, conn: &mut impl db::BackendConnection) -> Result<()> {
        let backend_name = conn.backend_name();
        let tx = conn.transaction().await?;
        let sql = self
            .up_sql(backend_name)?
            .ok_or_else(|| Error::UnknownBackend(backend_name.to_string()))?;
        tx.execute(&sql).await?;
        self.mark_applied(&tx).await?;
        tx.commit().await
    }

    /// Mark the migration as being applied without doing any
    /// work. Use carefully -- the caller must ensure that the
    /// database schema already matches that expected by this
    /// migration.
    async fn mark_applied(&self, conn: &impl db::ConnectionMethods) -> Result<()> {
        conn.insert_only(
            ButaneMigration::TABLE,
            ButaneMigration::COLUMNS,
            &[self.name().as_ref().to_sql_ref()],
        )
        .await
    }

    /// Un-apply (downgrade) the migration to a database
    /// connection. The connection must be for the same type of
    /// database as this and this must be the latest migration applied
    /// to the database.
    async fn downgrade(&self, conn: &mut impl db::BackendConnection) -> Result<()> {
        let backend_name = conn.backend_name();
        let tx = conn.transaction().await?;
        let sql = self
            .down_sql(backend_name)?
            .ok_or_else(|| Error::UnknownBackend(backend_name.to_string()))?;
        tx.execute(&sql).await?;
        let nameval = self.name().as_ref().to_sql();
        tx.delete_where(
            ButaneMigration::TABLE,
            BoolExpr::Eq(ButaneMigration::PKCOL, Expr::Val(nameval)),
        )
        .await?;
        tx.commit().await
    }
}

/// A migration which can be modified
pub trait MigrationMut: Migration {
    /// Adds an abstract table to the migration. The table state should
    /// represent the expected state after the migration has been
    /// applied. It is expected that all tables will be added to the
    /// migration in this fashion, if they were modified in this migration.
    fn add_modified_table(&mut self, table: &ATable) -> Result<()>;

    /// Marks a table as not modified in this migration.
    /// Use instead of `add_modified_table`.
    #[allow(unused_variables)]
    fn add_unmodified_table(&mut self, table: &ATable, from_migration_name: &str) -> Result<()>;

    /// Delete the table with the given name. Note that simply
    /// deleting a table in code does not work -- it will remain with
    /// its last known schema unless explicitly deleted. See also the
    /// butane cli command `butane delete table <TABLE>`.
    fn delete_table(&mut self, name: &str) -> Result<()>;

    /// Set the backend-specific commands to apply/undo this migration.
    fn add_sql(&mut self, backend_name: &str, up_sql: &str, down_sql: &str) -> Result<()>;

    /// Remove the backend-specific commands to apply/undo this migration.
    fn remove_sql(&mut self, backend_name: &str) -> Result<()>;

    /// Adds a TypeKey -> SqlType mapping. Only meaningful on the special current migration.
    fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()>;

    /// Set the name of the migration before this one.
    fn set_migration_from(&mut self, prev: Option<String>) -> Result<()>;
}
