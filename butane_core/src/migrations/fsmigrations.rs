use super::adb::{ATable, DeferredSqlType, TypeKey, ADB};
use super::fs::{Filesystem, OsFilesystem};
use super::{Migration, MigrationMut, Migrations, MigrationsMut};
use crate::{ConnectionMethods, DataObject, Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

type SqlTypeMap = BTreeMap<TypeKey, DeferredSqlType>;
const TYPES_FILENAME: &str = "types.json";

#[derive(Debug, Deserialize, Serialize)]
struct MigrationInfo {
    /// The migration this one is based on, or None if this is the
    /// first migration in the chain
    from_name: Option<String>,
    backends: Vec<String>,
}
impl MigrationInfo {
    fn new() -> Self {
        MigrationInfo {
            from_name: None,
            backends: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct MigrationsState {
    latest: Option<String>,
}
impl MigrationsState {
    fn new() -> Self {
        MigrationsState { latest: None }
    }
}

/// A migration stored in the filesystem
#[derive(Clone, Debug)]
pub struct FsMigration {
    fs: Rc<dyn Filesystem>,
    root: PathBuf,
}

impl FsMigration {
    pub fn copy_to(&self, other: &mut Self) -> Result<()> {
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

    fn write_info(&self, info: &MigrationInfo) -> Result<()> {
        self.write_contents("info.json", serde_json::to_string(info)?.as_bytes())
    }

    fn write_sql(&self, name: &str, sql: &str) -> Result<()> {
        self.write_contents(&format!("{name}.sql"), sql.as_bytes())
    }

    fn read_sql(&self, backend: &str, direction: &str) -> Result<Option<String>> {
        let path = self.sql_path(backend, direction);
        let mut buf = String::new();
        if !path.exists() {
            return Ok(None);
        }
        self.fs.read(&path)?.read_to_string(&mut buf)?;
        Ok(Some(buf))
    }

    fn sql_path(&self, backend: &str, direction: &str) -> PathBuf {
        self.root.join(format!("{backend}_{direction}.sql"))
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
        Ok(self.fs.ensure_dir(&self.root)?)
    }

    fn info(&self) -> Result<MigrationInfo> {
        let path = self.root.join("info.json");
        if !path.exists() {
            return Ok(MigrationInfo::new());
        }
        let info: MigrationInfo = serde_json::from_reader(self.fs.read(&path)?)?;
        Ok(info)
    }

    fn lock_exclusive(&self) -> Result<MigrationLock> {
        MigrationLock::new_exclusive(&self.root.join("lock"))
    }

    fn lock_shared(&self) -> Result<MigrationLock> {
        MigrationLock::new_shared(&self.root.join("lock"))
    }
}

impl MigrationMut for FsMigration {
    fn write_table(&mut self, table: &ATable) -> Result<()> {
        self.write_contents(
            &format!("{}.table", table.name),
            serde_json::to_string(table)?.as_bytes(),
        )
    }

    fn delete_table(&mut self, table: &str) -> Result<()> {
        let fname = format!("{table}.table");
        self.ensure_dir()?;
        let path = self.root.join(fname);
        std::fs::remove_file(path)?;
        Ok(())
    }

    fn add_sql(&mut self, backend_name: &str, up_sql: &str, down_sql: &str) -> Result<()> {
        self.write_sql(&format!("{backend_name}_up"), up_sql)?;
        self.write_sql(&format!("{backend_name}_down"), down_sql)?;
        let mut info = self.info()?;
        info.backends.push(backend_name.to_string());
        self.write_info(&info)?;
        Ok(())
    }

    fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()> {
        let _lock = self.lock_exclusive();
        let typefile = self.root.join(TYPES_FILENAME);

        let mut types: SqlTypeMap = match self.fs.read(&typefile) {
            Ok(reader) => serde_json::from_reader(reader).map_err(|e| {
                eprintln!("failed to read types {typefile:?}");
                e
            })?,
            Err(_) => BTreeMap::new(),
        };
        types.insert(key, sqltype);
        self.write_contents(
            TYPES_FILENAME,
            serde_json::to_string(&types)
                .map_err(|e| {
                    eprintln!("failed to read types");
                    e
                })?
                .as_bytes(),
        )?;
        Ok(())
    }

    /// Set the migration before this one.
    fn set_migration_from(&mut self, prev: Option<String>) -> Result<()> {
        let mut info = self.info()?;
        info.from_name = prev;
        self.write_info(&info)
    }
}

impl Migration for FsMigration {
    fn db(&self) -> Result<ADB> {
        let _lock = self.lock_shared()?;
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

    fn migration_from(&self) -> Result<Option<Cow<str>>> {
        Ok(self.info()?.from_name.map(Cow::from))
    }

    fn name(&self) -> Cow<str> {
        // There should be no way our root has no name portion
        self.root.file_name().unwrap().to_string_lossy()
    }

    fn up_sql(&self, backend_name: &str) -> Result<Option<String>> {
        self.read_sql(backend_name, "up")
    }

    fn down_sql(&self, backend_name: &str) -> Result<Option<String>> {
        self.read_sql(backend_name, "down")
    }

    fn sql_backends(&self) -> Result<Vec<String>> {
        Ok(self.info()?.backends)
    }
}

impl PartialEq for FsMigration {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for FsMigration {}

/// A collection of migrations stored in the filesystem.
#[derive(Debug)]
pub struct FsMigrations {
    fs: Rc<dyn Filesystem>,
    root: PathBuf,
    current: FsMigration,
}
impl FsMigrations {
    pub fn new(root: PathBuf) -> Self {
        let fs = Rc::new(OsFilesystem {});
        let current = FsMigration {
            fs: fs.clone(),
            root: root.join("current"),
        };
        FsMigrations { fs, root, current }
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
    fn save_state(&mut self, state: &MigrationsState) -> Result<()> {
        let path = self.root.join("state.json");
        let mut f = self.fs.write(&path)?;
        f.write_all(serde_json::to_string(state)?.as_bytes())
            .map_err(|e| e.into())
    }
    /// Detach the latest migration from the list of migrations,
    /// leaving the migration on the filesystem.
    pub fn detach_latest_migration(&mut self) -> Result<()> {
        let latest = self
            .latest()
            .ok_or(Error::MigrationError("There are no migrations".to_string()))?;
        let from_name =
            latest
                .migration_from()?
                .map(|s| s.to_string())
                .ok_or(Error::MigrationError(
                    "There is no previous migration".to_string(),
                ))?;
        let mut state = self.get_state()?;
        state.latest = Some(from_name);
        self.save_state(&state)?;
        Ok(())
    }
    /// Provides a Vec of migration directories that have been detached.
    pub fn detached_migration_paths(&self) -> Result<Vec<String>> {
        let migration_series = self.all_migrations()?;
        let mut detached_directory_names: Vec<String> = vec![];
        for entry in std::fs::read_dir(self.root.clone())? {
            let path = entry?.path();
            let name = path.file_name().unwrap();
            if !path.is_dir() || name == "current" {
                continue;
            }
            if !migration_series.iter().any(|item| item.root == path) {
                detached_directory_names.push(path.display().to_string());
            };
        }
        Ok(detached_directory_names)
    }
}

impl Migrations for FsMigrations {
    type M = FsMigration;

    fn get_migration(&self, name: &str) -> Option<Self::M> {
        let mut dir = self.root.clone();
        dir.push(name);
        if dir.exists() {
            Some(FsMigration {
                fs: self.fs.clone(),
                root: dir,
            })
        } else {
            None
        }
    }

    fn latest(&self) -> Option<Self::M> {
        self.get_state().map_or(None, |state| match state.latest {
            None => None,
            Some(name) => self.get_migration(&name),
        })
    }
}

impl MigrationsMut for FsMigrations {
    fn current(&mut self) -> &mut Self::M {
        &mut self.current
    }
    fn clear_current(&mut self) -> Result<()> {
        std::fs::remove_dir_all(&self.current.root)?;
        Ok(())
    }
    fn new_migration(&self, name: &str) -> Self::M {
        let mut dir = self.root.clone();
        dir.push(name);
        FsMigration {
            fs: self.fs.clone(),
            root: dir,
        }
    }
    fn add_migration(&mut self, m: Self::M) -> Result<()> {
        // Update state
        let from_name = m.migration_from()?.map(|s| s.to_string());
        let mut state = self.get_state()?;
        if state.latest.is_none() || state.latest == from_name {
            state.latest = Some(m.name().to_string());
            self.save_state(&state)?;
        }
        Ok(())
    }

    fn clear_migrations(&mut self, conn: &impl ConnectionMethods) -> Result<()> {
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if matches!(entry.path().file_name(), Some(name) if name == "current") {
                continue;
            }
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        conn.delete_where(super::ButaneMigration::TABLE, crate::query::BoolExpr::True)?;
        Ok(())
    }
}

#[derive(Debug)]
struct MigrationLock {
    file: File,
}
impl MigrationLock {
    fn new_exclusive(path: &Path) -> Result<Self> {
        let file = Self::get_file(path)?;
        file.lock_exclusive()?;
        Ok(MigrationLock { file })
    }

    fn new_shared(path: &Path) -> Result<Self> {
        let file = Self::get_file(path)?;
        file.lock_shared()?;
        Ok(MigrationLock { file })
    }

    fn get_file(path: &Path) -> Result<File> {
        Ok(OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?)
    }
}
impl Drop for MigrationLock {
    fn drop(&mut self) {
        self.file.unlock().unwrap();
    }
}
