use crate::adb;
use crate::adb::*;
use crate::db;
use crate::Result;
use failure::format_err;
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub use crate::adb::ADB;

#[derive(Serialize, Deserialize)]
struct MigrationInfo {
    /// The migration this one is based on, or None if this is the
    /// first migration in the chain
    from_name: Option<String>,
}

#[derive(Clone)]
pub struct Migration {
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
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_name().to_string_lossy().ends_with(".table") {
                continue;
            }
            let table: ATable = serde_json::from_reader(fs::File::open(entry.path())?)?;
            db.replace_table(table)
        }
        Ok(db)
    }

    /// Get the migration before this one (if any).
    pub fn get_from_migration(&self) -> Result<Option<Migration>> {
        let info: MigrationInfo =
            serde_json::from_reader(fs::File::open(self.root.join("info.json"))?)?;
        match info.from_name {
            None => Ok(None),
            Some(name) => {
                let m = from_root(
                    self.root
                        .parent()
                        .ok_or(format_err!("migration path must have a parent"))?,
                )
                .get_migration(&name);
                Ok(Some(m))
            }
        }
    }

    pub fn get_name(&self) -> Cow<str> {
        // There should be no way our root has no name portion
        self.root.file_name().unwrap().to_string_lossy()
    }

    pub fn get_up_sql(&self, backend_name: &str) -> Result<String> {
        self.read_sql(&format!("{}_up", backend_name))
    }

    pub fn get_down_sql(&self, backend_name: &str) -> Result<String> {
        self.read_sql(&format!("{}_down", backend_name))
    }

    fn write_info(&self, info: &MigrationInfo) -> Result<()> {
        self.write_contents("info.json", serde_json::to_string(info)?.as_bytes())
    }

    fn write_sql(&self, name: &str, sql: &str) -> Result<()> {
        self.write_contents(&format!("{}.sql", name), sql.as_bytes())
    }

    fn read_sql(&self, name: &str) -> Result<String> {
        let mut buf = String::new();
        fs::File::open(self.root.join(&format!("{}.sql", name)))?.read_to_string(&mut buf)?;
        Ok(buf)
    }

    fn write_contents(&self, fname: &str, contents: &[u8]) -> Result<()> {
        self.ensure_dir()?;
        let path = self.root.join(fname);
        let mut f = fs::File::create(path)?;
        f.write_all(contents).map_err(|e| e.into())
    }

    fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root).map_err(|e| e.into())
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
    root: PathBuf,
}
impl Migrations {
    pub fn get_migration(&self, name: &str) -> Migration {
        let mut dir = self.root.clone();
        dir.push(name);
        Migration { root: dir }
    }

    pub fn get_current(&self) -> Migration {
        self.get_migration("current")
    }

    pub fn get_latest(&self) -> Option<Migration> {
        self.get_state()
            .map(|state| match state.latest {
                None => None,
                Some(name) => Some(self.get_migration(&name)),
            })
            .unwrap_or(None)
    }

    /// Create a migration `from` -> `to` from may be None, in which
    /// case the migration is created from an empty database.
    /// Returns None if `from` and `to` represent identical states
    pub fn create_migration_sql(
        &self,
        backend: impl db::Backend,
        name: &str,
        from: Option<Migration>,
        to: &Migration,
    ) -> Result<Option<Migration>> {
        let empty_db = Ok(ADB::new());
        let from_name = from.as_ref().map(|m| m.get_name().to_string());
        let from_db = from.map_or(empty_db, |m| m.get_db())?;
        let to_db = to.get_db()?;
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
        Err(format_err!("Migration not in chain"))
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

    fn get_state(&self) -> Result<MigrationsState> {
        let path = self.root.join("state.json");
        if !path.exists() {
            return Ok(MigrationsState::new());
        }
        serde_json::from_reader(fs::File::open(path)?).map_err(|e| e.into())
    }

    fn save_state(&self, state: &MigrationsState) -> Result<()> {
        let path = self.root.join("state.json");
        let mut f = fs::File::create(path)?;
        f.write_all(serde_json::to_string(state)?.as_bytes())
            .map_err(|e| e.into())
    }
}

pub fn from_root<P: AsRef<Path>>(path: P) -> Migrations {
    Migrations {
        root: path.as_ref().to_path_buf(),
    }
}
