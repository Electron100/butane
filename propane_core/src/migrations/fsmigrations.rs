use super::adb;
use super::adb::{ATable, DeferredSqlType, Operation, TypeKey, ADB};
use super::fs::Filesystem;
use super::{
    migrations_table, Migration, MigrationInfo, MigrationMut, Migrations, MigrationsMut,
    MigrationsState, PropaneMigration,
};
use crate::db::internal::ConnectionMethods;
use crate::sqlval::ToSql;
use crate::{db, DataObject, DataResult, Error, Result};
use serde_json;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

type SqlTypeMap = BTreeMap<TypeKey, DeferredSqlType>;
const TYPES_FILENAME: &'static str = "types.json";

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

    fn add_type(&self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()> {
        let mut types: SqlTypeMap = match self.fs.read(&self.root.join(TYPES_FILENAME)) {
            Ok(reader) => serde_json::from_reader(reader)?,
            Err(_) => BTreeMap::new(),
        };
        types.insert(key, sqltype);
        self.write_contents(TYPES_FILENAME, serde_json::to_string(&types)?.as_bytes())?;
        return Ok(());
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

    fn migration_from(&self) -> Result<Option<Self>> {
        let info: MigrationInfo =
            serde_json::from_reader(self.fs.read(&self.root.join("info.json"))?)?;
        match info.from_name {
            None => Ok(None),
            Some(name) => {
                let mut dir = self
                    .root
                    .parent()
                    .ok_or_else(|| {
                        Error::MigrationError("migration path must have a parent".to_string())
                    })?
                    .to_path_buf();
                dir.push(name);
                Ok(Some(FsMigration {
                    root: dir,
                    fs: self.fs.clone(),
                }))
            }
        }
    }

    fn name(&self) -> Cow<str> {
        // There should be no way our root has no name portion
        self.root.file_name().unwrap().to_string_lossy()
    }

    fn apply(&self, conn: &mut impl db::BackendConnection) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(&self.up_sql(tx.backend_name())?)?;
        tx.insert_or_replace(
            PropaneMigration::TABLE,
            PropaneMigration::COLUMNS,
            &[self.name().as_ref().to_sql()],
        )?;
        tx.commit()
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
    pub(crate) fs: Rc<dyn Filesystem>,
    pub(crate) root: PathBuf,
}
impl FsMigrations {}

impl Migrations for FsMigrations {
    type M = FsMigration;

    fn get_migration(&self, name: &str) -> Self::M {
        let mut dir = self.root.clone();
        dir.push(name);
        FsMigration {
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
}

impl MigrationsMut for FsMigrations {
    fn create_migration(
        &self,
        backend: &impl db::Backend,
        name: &str,
        from: Option<Self::M>,
    ) -> Result<Option<Self::M>> {
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
        let mut m = self.get_migration(name);
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

    fn save_state(&self, state: &MigrationsState) -> Result<()> {
        let path = self.root.join("state.json");
        let mut f = self.fs.write(&path)?;
        f.write_all(serde_json::to_string(state)?.as_bytes())
            .map_err(|e| e.into())
    }
}
