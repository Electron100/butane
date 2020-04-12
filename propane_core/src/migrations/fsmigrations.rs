use super::adb::{ATable, DeferredSqlType, TypeKey, ADB};
use super::fs::{Filesystem, OsFilesystem};
use super::{Migration, MigrationMut, Migrations, MigrationsMut};
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::BTreeMap;

use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

type SqlTypeMap = BTreeMap<TypeKey, DeferredSqlType>;
const TYPES_FILENAME: &str = "types.json";

#[derive(Serialize, Deserialize)]
struct MigrationInfo {
    /// The migration this one is based on, or None if this is the
    /// first migration in the chain
    from_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MigrationsState {
    latest: Option<String>,
}
impl MigrationsState {
    fn new() -> Self {
        MigrationsState { latest: None }
    }
}

/// A migration stored in the filesystem
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
        self.write_contents(&format!("{}.sql", name), sql.as_bytes())
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
        Ok(self.fs.ensure_dir(&self.root)?)
    }
}

impl MigrationMut for FsMigration {
    fn write_table(&mut self, table: &ATable) -> Result<()> {
        self.write_contents(
            &format!("{}.table", table.name),
            serde_json::to_string(table)?.as_bytes(),
        )
    }

    fn add_up_sql(&mut self, backend_name: &str, sql: &str) -> Result<()> {
        self.write_sql(&format!("{}_up", backend_name), &sql)
    }

    fn add_down_sql(&mut self, backend_name: &str, sql: &str) -> Result<()> {
        self.write_sql(&format!("{}_down", backend_name), &sql)
    }

    fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()> {
        let mut types: SqlTypeMap = match self.fs.read(&self.root.join(TYPES_FILENAME)) {
            Ok(reader) => serde_json::from_reader(reader)?,
            Err(_) => BTreeMap::new(),
        };
        types.insert(key, sqltype);
        self.write_contents(TYPES_FILENAME, serde_json::to_string(&types)?.as_bytes())?;
        Ok(())
    }

    /// Set the migration before this one.
    fn set_migration_from(&mut self, prev: Option<String>) -> Result<()> {
        self.write_info(&MigrationInfo { from_name: prev })
    }
}

impl Migration for FsMigration {
    fn db(&self) -> Result<ADB> {
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
        let path = self.root.join("info.json");
        if !path.exists() {
            return Ok(None);
        }
        let info: MigrationInfo = serde_json::from_reader(self.fs.read(&path)?)?;
        Ok(info.from_name.map(|name| Cow::from(name)))
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
}

impl PartialEq for FsMigration {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for FsMigration {}

/// A collection of migrations.
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
        self.get_state()
            .map(|state| match state.latest {
                None => None,
                Some(name) => self.get_migration(&name),
            })
            .unwrap_or(None)
    }
}

impl MigrationsMut for FsMigrations {
    fn current(&mut self) -> &mut Self::M {
        &mut self.current
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
}
