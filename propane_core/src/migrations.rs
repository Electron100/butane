use crate::adb;
use crate::adb::*;
use crate::db;
use crate::Result;
use serde_json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use crate::adb::ADB;

pub struct Migration {
    db: Option<ADB>,
    root: PathBuf,
}
impl Migration {
    pub fn write_table(&self, table: &ATable) -> Result<()> {
        self.write_contents(
            &format!("{}.table", table.name),
            serde_json::to_string(table)?.as_bytes(),
        )
    }

    fn write_sql(&self, backend_name: &str, sql: &str) -> Result<()> {
        self.write_contents(&format!("{}.sql", backend_name), sql.as_bytes())
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

pub struct Migrations {
    root: PathBuf,
}
impl Migrations {
    pub fn get_migration(&self, name: &str) -> Migration {
        let mut dir = self.root.clone();
        dir.push(name);
        Migration {
            db: None,
            root: dir,
        }
    }

    pub fn get_current(&self) -> Migration {
        self.get_migration("current")
    }

    pub fn get_latest(&self) -> Option<Migration> {
        self.get_latest_helper().unwrap_or(None)
    }

    pub fn create_migration_sql(
        &self,
        name: &str,
        backend: impl db::Backend,
        from: &ADB,
        to: &ADB,
    ) -> Result<Migration> {
        let sql = backend.create_migration_sql(from, &adb::diff(from, to));
        let m = self.get_migration(name);
        m.write_sql(backend.get_name(), &sql)?;
        Ok(m)
    }

    fn get_latest_helper(&self) -> std::io::Result<Option<Migration>> {
        let mut names: Vec<String> = fs::read_dir(&self.root)?
            .filter_map(|entry| {
                if entry.is_err() {
                    return None;
                }
                let entry = entry.unwrap();
                if let Ok(ty) = entry.file_type() {
                    if ty.is_dir() {
                        if let Ok(s) = entry.file_name().into_string() {
                            return Some(s);
                        }
                    }
                }
                None
            })
            .collect();
        names.sort();
        match names.last() {
            Some(name) => Ok(Some(self.get_migration(name))),
            _ => Ok(None),
        }
    }
}

pub fn from_root<P: AsRef<Path>>(path: P) -> Migrations {
    Migrations {
        root: path.as_ref().to_path_buf(),
    }
}
