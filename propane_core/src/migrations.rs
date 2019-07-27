use crate::adb;
pub use crate::adb::ADB;
use crate::adb::*;
use crate::db;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
        std::fs::File::create(path).map(|f| Box::new(f) as Box<Write>)
    }
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>> {
        std::fs::File::open(path).map(|f| Box::new(f) as Box<Read>)
    }
}

#[derive(Serialize, Deserialize)]
struct MigrationInfo {
    /// The migration this one is based on, or None if this is the
    /// first migration in the chain
    from_name: Option<String>,
}

pub struct Migration {
    fs: Rc<Filesystem>,
    root: PathBuf,
}
impl Migration {
    pub fn write_table(&self, table: &ATable) -> Result<()> {
        self.write_contents(
            &format!("{}.table", table.name),
            serde_json::to_string(table)?.as_bytes(),
        )
    }

    pub fn get_db(&self) -> Result<ADB> {
        let mut db = ADB::new();
        self.ensure_dir()?;
        let entries = self.fs.list_dir(&self.root)?;
        for entry in entries {
            match entry.file_name() {
                None => continue,
                Some(name) => {
                    if !name.to_string_lossy().ends_with(".table") {
                        continue;
                    }
                }
            }
            let table: ATable = serde_json::from_reader(self.fs.read(&entry)?)?;
            db.replace_table(table)
        }
        db.resolve_types()?;
        Ok(db)
    }

    /// Get the migration before this one (if any).
    pub fn get_from_migration(&self) -> Result<Option<Migration>> {
        let info: MigrationInfo =
            serde_json::from_reader(self.fs.read(&self.root.join("info.json"))?)?;
        match info.from_name {
            None => Ok(None),
            Some(name) => {
                let m = from_root(self.root.parent().ok_or(Error::MigrationError(
                    "migration path must have a parent".to_string(),
                ))?)
                .get_migration(&name);
                Ok(Some(m))
            }
        }
    }

    pub fn get_name(&self) -> Cow<str> {
        // There should be no way our root has no name portion
        self.root.file_name().unwrap().to_string_lossy()
    }

    pub fn up_sql(&self, backend_name: &str) -> Result<String> {
        self.read_sql(backend_name, "up")
    }

    pub fn down_sql(&self, backend_name: &str) -> Result<String> {
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
        self.get_name() == other.get_name()
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

pub struct Migrations {
    fs: Rc<Filesystem>,
    root: PathBuf,
}
impl Migrations {
    /// Get a migration representing the current state as determined
    /// by the last build of models. This does not necessarily match
    /// the current state of the database if migrations have not yet been applied.
    ///
    /// This migration is named "current".
    pub fn get_current(&self) -> Migration {
        self.get_migration("current")
    }

    /// Get the most recent migration other than "current" or None if
    /// no migrations have been created.
    pub fn get_latest(&self) -> Option<Migration> {
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
        let from_name = from.as_ref().map(|m| m.get_name().to_string());
        let from_db = from.map_or(empty_db, |m| m.get_db())?;
        let to_db = self.get_current().get_db()?;
        let ops = &adb::diff(&from_db, &to_db);
        if ops.is_empty() {
            return Ok(None);
        }
        let sql = backend.create_migration_sql(&from_db, ops);
        let m = self.get_migration(name);
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
            state.latest = Some(m.get_name().to_string());
            self.save_state(&state)?;
        }

        Ok(Some(m))
    }

    pub fn get_migrations_since(&self, since: &Migration) -> Result<Vec<Migration>> {
        let mut last = self.get_latest();
        let mut accum: Vec<Migration> = Vec::new();
        while let Some(m) = last {
            if m != *since {
                last = m.get_from_migration()?;
                accum.push(m);
                continue;
            }

            return Ok(accum.into_iter().rev().collect());
        }
        Err(Error::MigrationError("Migration not in chain".to_string()))
    }

    pub fn get_all_migrations(&self) -> Result<Vec<Migration>> {
        let mut last = self.get_latest();
        let mut accum: Vec<Migration> = Vec::new();
        while let Some(m) = last {
            last = m.get_from_migration()?;
            accum.push(m);
        }
        Ok(accum.into_iter().rev().collect())
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

pub fn from_root_and_filesystem<P: AsRef<Path>>(
    path: P,
    fs: impl Filesystem + 'static,
) -> Migrations {
    Migrations {
        fs: Rc::new(fs),
        root: path.as_ref().to_path_buf(),
    }
}

pub fn from_root<P: AsRef<Path>>(path: P) -> Migrations {
    from_root_and_filesystem(path, OsFilesystem {})
}
