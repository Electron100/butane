//! For working with migrations. If using the propane CLI tool, it is
//! not necessary to use these types directly.

use crate::db::internal::{Column, ConnectionMethods, Row};
use crate::sqlval::{FromSql, SqlVal, ToSql};
use crate::{db, query, DataObject, DataResult, Error, Result, SqlType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub mod adb;
use adb::{AColumn, ATable, DeferredSqlType, Operation, TypeKey, ADB};

#[cfg(feature = "memfs")]
pub mod memfs;

const TYPES_FILENAME: &'static str = "types.json";

/// Filesystem abstraction for `Migrations`. Primarily intended to
/// allow bypassing the real filesystem during testing, but
/// implementations that do not call through to the real filesystem
/// are supported in production.
pub trait Filesystem {
    /// Ensure a directory exists, recursively creating missing components
    fn ensure_dir(&self, path: &Path) -> std::io::Result<()>;
    /// List all paths in a directory
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>>;
    /// Opens a file for writing. Creates it if it does not exist. Truncates it otherwise.
    fn write(&self, path: &Path) -> std::io::Result<Box<dyn Write>>;
    /// Opens a file for reading.
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>>;
}

struct OsFilesystem {}

impl Filesystem for OsFilesystem {
    fn ensure_dir(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>> {
        std::fs::read_dir(path)?
            .map(|entry| entry.map(|de| de.path()))
            .collect()
    }
    fn write(&self, path: &Path) -> std::io::Result<Box<dyn Write>> {
        std::fs::File::create(path).map(|f| Box::new(f) as Box<dyn Write>)
    }
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>> {
        std::fs::File::open(path).map(|f| Box::new(f) as Box<dyn Read>)
    }
}

#[derive(Serialize, Deserialize)]
struct MigrationInfo {
    /// The migration this one is based on, or None if this is the
    /// first migration in the chain
    from_name: Option<String>,
}

type SqlTypeMap = BTreeMap<TypeKey, DeferredSqlType>;

/// Type representing a database migration. A migration describes how
/// to bring the database from state A to state B. In general, the
/// methods on this type are persistent -- they read from and write to
/// the filesystem.
///
/// A Migration cannot be constructed directly, only retrieved from
/// [Migrations][crate::migrations::Migrations].
pub struct Migration {
    fs: Rc<dyn Filesystem>,
    root: PathBuf,
}
impl Migration {
    /// Adds an abstract table to the migration. The table state should
    /// represent the expected state after the migration has been
    /// applied. It is expected that all tables will be added to the
    /// migration in this fashion.
    pub fn write_table(&self, table: &ATable) -> Result<()> {
        self.write_contents(
            &format!("{}.table", table.name),
            serde_json::to_string(table)?.as_bytes(),
        )
    }

    /// Adds a TypeKey -> SqlType mapping. Only meaningful on the special current migration.
    pub fn add_type(&self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()> {
        let mut types: SqlTypeMap = match self.fs.read(&self.root.join(TYPES_FILENAME)) {
            Ok(reader) => serde_json::from_reader(reader)?,
            Err(_) => BTreeMap::new(),
        };
        types.insert(key, sqltype);
        self.write_contents(TYPES_FILENAME, serde_json::to_string(&types)?.as_bytes())?;
        return Ok(());
    }

    /// Retrieves the full abstract database state describing all tables
    pub fn db(&self) -> Result<ADB> {
        let mut db = ADB::new();
        self.ensure_dir()?;
        let entries = self.fs.list_dir(&self.root)?;
        for entry in entries {
            match entry.file_name() {
                None => continue,
                Some(name) => {
                    let name = name.to_string_lossy();
                    if name.ends_with(".table") {
                        let table: ATable = serde_json::from_reader(self.fs.read(&entry)?)?;
                        db.replace_table(table)
                    } else if name == TYPES_FILENAME {
                        let types: SqlTypeMap = serde_json::from_reader(
                            self.fs.read(&self.root.join(TYPES_FILENAME))?,
                        )?;

                        for (key, sqltype) in types {
                            db.add_type(key, sqltype);
                        }
                    }
                }
            }
        }
        db.resolve_types()?;
        Ok(db)
    }

    pub fn copy_to(&self, other: &mut Migration) -> Result<()> {
        self.ensure_dir()?;
        other.ensure_dir()?;
        let entries = self.fs.list_dir(&self.root)?;
        for entry in entries {
            match entry.file_name() {
                None => continue,
                Some(name) => {
                    let mut rd = self.fs.read(&entry)?;
                    let mut wr = other.fs.write(&other.root.join(name))?;
                    std::io::copy(&mut rd, &mut wr)?;
                }
            }
        }
        Ok(())
    }

    /// Get the migration before this one (if any).
    #[allow(clippy::wrong_self_convention)]
    pub fn from_migration(&self) -> Result<Option<Migration>> {
        let info: MigrationInfo =
            serde_json::from_reader(self.fs.read(&self.root.join("info.json"))?)?;
        match info.from_name {
            None => Ok(None),
            Some(name) => {
                let m = from_root(self.root.parent().ok_or_else(|| {
                    Error::MigrationError("migration path must have a parent".to_string())
                })?)
                .get_migration(&name);
                Ok(Some(m))
            }
        }
    }

    /// The name of this migration.
    pub fn name(&self) -> Cow<str> {
        // There should be no way our root has no name portion
        self.root.file_name().unwrap().to_string_lossy()
    }

    /// Apply the migration to a database connection. The connection
    /// must be for the same type of database as
    /// [create_migration][crate::migrations::Migrations::create_migration]
    /// and the database must be in the state of the migration prior
    /// to this one ([from_migration][crate::migrations::Migration::from_migration])
    pub fn apply(&self, conn: &mut impl db::BackendConnection) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(&self.up_sql(tx.backend_name())?)?;
        tx.insert_or_replace(
            PropaneMigration::TABLE,
            PropaneMigration::COLUMNS,
            &[self.name().as_ref().to_sql()],
        )?;
        tx.commit()
    }

    fn up_sql(&self, backend_name: &str) -> Result<String> {
        self.read_sql(backend_name, "up")
    }

    #[allow(dead_code)] // TODO use this, shouldn't be dead
    fn down_sql(&self, backend_name: &str) -> Result<String> {
        self.read_sql(backend_name, "down")
    }

    fn write_info(&self, info: &MigrationInfo) -> Result<()> {
        self.write_contents("info.json", serde_json::to_string(info)?.as_bytes())
    }

    fn write_sql(&self, name: &str, sql: &str) -> Result<()> {
        self.write_contents(&format!("{}.sql", name), sql.as_bytes())
    }

    fn read_sql(&self, backend: &str, direction: &str) -> Result<String> {
        let path = self.sql_path(backend, direction);
        let mut buf = String::new();
        self.fs.read(&path)?.read_to_string(&mut buf)?;
        Ok(buf)
    }

    fn sql_path(&self, backend: &str, direction: &str) -> PathBuf {
        self.root.join(&format!("{}_{}.sql", backend, direction))
    }

    fn write_contents(&self, fname: &str, contents: &[u8]) -> Result<()> {
        self.ensure_dir()?;
        let path = self.root.join(fname);
        self.fs
            .write(&path)?
            .write_all(contents)
            .map_err(|e| e.into())
    }

    fn ensure_dir(&self) -> Result<()> {
        self.fs.ensure_dir(&self.root).map_err(|e| e.into())
    }
}
impl PartialEq for Migration {
    fn eq(&self, other: &Migration) -> bool {
        self.name() == other.name()
    }
}
impl Eq for Migration {}

#[derive(Serialize, Deserialize)]
struct MigrationsState {
    latest: Option<String>,
}
impl MigrationsState {
    fn new() -> Self {
        MigrationsState { latest: None }
    }
}

/// A collection of migrations.
pub struct Migrations {
    fs: Rc<dyn Filesystem>,
    root: PathBuf,
}
impl Migrations {
    /// Get a pseudo-migration representing the current state as
    /// determined by the last build of models. This does not
    /// necessarily match the current state of the database if
    /// migrations have not yet been applied.
    ///
    /// This migration is named "current". It is not a "real" migration
    /// - it should never be applied
    /// - it will never be returned by `latest`, `migrations_since`, `all_migrations` or other similar methods.
    pub fn current(&self) -> Migration {
        self.get_migration("current")
    }

    /// Get the most recent migration other than `current` or `None` if
    /// no migrations have been created.
    pub fn latest(&self) -> Option<Migration> {
        self.get_state()
            .map(|state| match state.latest {
                None => None,
                Some(name) => Some(self.get_migration(&name)),
            })
            .unwrap_or(None)
    }

    /// Create a migration `from` -> `current` named `name`. From may be None, in which
    /// case the migration is created from an empty database.
    /// Returns None if `from` and `current` represent identical states
    pub fn create_migration(
        &self,
        backend: &impl db::Backend,
        name: &str,
        from: Option<Migration>,
    ) -> Result<Option<Migration>> {
        let empty_db = Ok(ADB::new());
        let from_name = from.as_ref().map(|m| m.name().to_string());
        let from_none = from.is_none();
        let from_db = from.map_or(empty_db, |m| m.db())?;
        let to_db = self.current().db()?;
        let mut ops = adb::diff(&from_db, &to_db);
        if ops.is_empty() {
            return Ok(None);
        }

        if from_none {
            // This is the first migration. Create the propane_migration table
            ops.push(Operation::AddTable(migrations_table()));
        }

        let sql = backend.create_migration_sql(&from_db, &ops);
        let m = self.get_migration(name);
        // Save the DB for use by other migrations from this one
        for table in to_db.tables() {
            m.write_table(table)?;
        }
        m.write_sql(&format!("{}_up", backend.get_name()), &sql)?;
        // And write the undo
        let sql = backend.create_migration_sql(&from_db, &adb::diff(&to_db, &from_db));
        m.write_sql(&format!("{}_down", backend.get_name()), &sql)?;
        m.write_info(&MigrationInfo {
            from_name: from_name.clone(),
        })?;

        // Update state
        let mut state = self.get_state()?;
        if state.latest.is_none() || state.latest == from_name {
            state.latest = Some(m.name().to_string());
            self.save_state(&state)?;
        }

        Ok(Some(m))
    }

    /// Returns migrations since the given migration.
    pub fn migrations_since(&self, since: &Migration) -> Result<Vec<Migration>> {
        let mut last = self.latest();
        let mut accum: Vec<Migration> = Vec::new();
        while let Some(m) = last {
            if m != *since {
                last = m.from_migration()?;
                accum.push(m);
                continue;
            }

            return Ok(accum.into_iter().rev().collect());
        }
        Err(Error::MigrationError("Migration not in chain".to_string()))
    }

    /// Returns all migrations
    pub fn all_migrations(&self) -> Result<Vec<Migration>> {
        let mut last = self.latest();
        let mut accum: Vec<Migration> = Vec::new();
        while let Some(m) = last {
            last = m.from_migration()?;
            accum.push(m);
        }
        Ok(accum.into_iter().rev().collect())
    }

    /// Get migrations which have not yet been applied to the database
    pub fn unapplied_migrations(&self, conn: &impl ConnectionMethods) -> Result<Vec<Migration>> {
        match self.last_applied_migration(conn)? {
            None => self.all_migrations(),
            Some(m) => self.migrations_since(&m),
        }
    }

    /// Get the last migration that has been applied to the database or None
    /// if no migrations have been applied
    pub fn last_applied_migration(
        &self,
        conn: &impl ConnectionMethods,
    ) -> Result<Option<Migration>> {
        if !conn.has_table(PropaneMigration::TABLE)? {
            return Ok(None);
        }
        let migrations: Result<Vec<PropaneMigration>> = conn
            .query(
                PropaneMigration::TABLE,
                PropaneMigration::COLUMNS,
                None,
                None,
            )?
            .into_iter()
            .map(PropaneMigration::from_row)
            .collect();
        let migrations = migrations?;

        let mut m_opt = self.latest();
        while let Some(m) = m_opt {
            if migrations.contains(&PropaneMigration {
                name: m.name().to_string(),
            }) {
                return Ok(Some(m));
            }
            m_opt = m.from_migration()?;
        }
        Ok(None)
    }

    fn get_migration(&self, name: &str) -> Migration {
        let mut dir = self.root.clone();
        dir.push(name);
        Migration {
            fs: self.fs.clone(),
            root: dir,
        }
    }

    fn get_state(&self) -> Result<MigrationsState> {
        let path = self.root.join("state.json");
        let fr = self.fs.read(&path);
        match fr {
            Ok(f) => serde_json::from_reader(f).map_err(|e| e.into()),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(MigrationsState::new())
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn save_state(&self, state: &MigrationsState) -> Result<()> {
        let path = self.root.join("state.json");
        let mut f = self.fs.write(&path)?;
        f.write_all(serde_json::to_string(state)?.as_bytes())
            .map_err(|e| e.into())
    }
}

fn migrations_table() -> ATable {
    let mut table = ATable::new("propane_migrations".to_string());
    let col = AColumn::new(
        "name",
        DeferredSqlType::Known(SqlType::Text),
        false, // nullable
        true,  // pk
        false, // auto
        None,
    );
    table.add_column(col);
    table
}

/// Like `from_root` except allows specifying an alternate filesystem
/// implementation. Intended primarily for testing purposes.
pub fn from_root_and_filesystem<P: AsRef<Path>>(
    path: P,
    fs: impl Filesystem + 'static,
) -> Migrations {
    Migrations {
        fs: Rc::new(fs),
        root: path.as_ref().to_path_buf(),
    }
}

/// Create a `Migrations` from a filesystem location. The `#[model]`
/// attribute will write migration information to a
/// `propane/migrations` directory under the project directory.
pub fn from_root<P: AsRef<Path>>(path: P) -> Migrations {
    from_root_and_filesystem(path, OsFilesystem {})
}

#[derive(PartialEq)]
struct PropaneMigration {
    name: String,
}
impl DataResult for PropaneMigration {
    type DBO = Self;
    type Fields = (); // we don't need Fields as we never filter
    const COLUMNS: &'static [Column] = &[Column::new("name", SqlType::Text)];
    fn from_row(row: Row) -> Result<Self> {
        if row.len() != 1usize {
            return Err(Error::BoundsError(
                "Row has the wrong number of columns for this DataResult".to_string(),
            ));
        }
        let mut it = row.into_iter();
        Ok(PropaneMigration {
            name: FromSql::from_sql(it.next().unwrap())?,
        })
    }
    fn query() -> query::Query<Self> {
        query::Query::new("propane_migrations")
    }
}
impl DataObject for PropaneMigration {
    type PKType = String;
    const PKCOL: &'static str = "name";
    const TABLE: &'static str = "propane_migrations";
    fn pk(&self) -> &String {
        &self.name
    }
    fn save(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
        let mut values: Vec<SqlVal> = Vec::with_capacity(2usize);
        values.push(self.name.to_sql());
        conn.insert_or_replace(Self::TABLE, <Self as DataResult>::COLUMNS, &values)
    }
    fn delete(&self, conn: &impl ConnectionMethods) -> Result<()> {
        conn.delete(Self::TABLE, Self::PKCOL, self.pk().to_sql())
    }
}
