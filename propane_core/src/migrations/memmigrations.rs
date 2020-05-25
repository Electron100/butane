use super::adb::{ATable, DeferredSqlType, TypeKey, ADB};
use super::{Migration, MigrationMut, Migrations, MigrationsMut};
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::HashMap;

type SqlTypeMap = HashMap<TypeKey, DeferredSqlType>;

#[derive(Clone, Serialize, Deserialize)]
pub struct MemMigration {
    name: String,
    db: ADB,
    from: Option<String>,
    up: HashMap<String, String>,
    down: HashMap<String, String>,
    types: SqlTypeMap,
}

impl MemMigration {
    fn new(name: String) -> Self {
        MemMigration {
            name,
            db: ADB::new(),
            from: None,
            up: HashMap::new(),
            down: HashMap::new(),
            types: SqlTypeMap::new(),
        }
    }
}

impl Migration for MemMigration {
    fn db(&self) -> Result<ADB> {
        Ok(self.db.clone())
    }

    fn migration_from(&self) -> Result<Option<Cow<str>>> {
        Ok(self.from.as_ref().map(Cow::from))
    }

    fn name(&self) -> Cow<str> {
        Cow::from(&self.name)
    }

    /// The backend-specific commands to apply this migration.
    fn up_sql(&self, backend_name: &str) -> Result<Option<String>> {
        Ok(self.up.get(backend_name).map(|s| s.to_string()))
    }

    /// The backend-specific commands to undo this migration.
    fn down_sql(&self, backend_name: &str) -> Result<Option<String>> {
        Ok(self.down.get(backend_name).map(|s| s.to_string()))
    }
    fn sql_backends(&self) -> Result<Vec<String>> {
        Ok(self.up.keys().map(|k| k.to_string()).collect())
    }
}
impl PartialEq for MemMigration {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for MemMigration {}

impl MigrationMut for MemMigration {
    fn write_table(&mut self, table: &ATable) -> Result<()> {
        self.db.replace_table(table.clone());
        Ok(())
    }
    fn add_sql(&mut self, backend_name: &str, up_sql: &str, down_sql: &str) -> Result<()> {
        self.up.insert(backend_name.to_string(), up_sql.to_string());
        self.down
            .insert(backend_name.to_string(), down_sql.to_string());
        Ok(())
    }
    fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) -> Result<()> {
        self.types.insert(key, sqltype);
        Ok(())
    }
    fn set_migration_from(&mut self, prev: Option<String>) -> Result<()> {
        self.from = prev;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct MemMigrations {
    migrations: HashMap<String, MemMigration>,
    current: MemMigration,
    latest: Option<String>,
}

impl MemMigrations {
    pub fn new() -> Self {
        MemMigrations {
            migrations: HashMap::new(),
            current: MemMigration::new("current".to_string()),
            latest: None,
        }
    }
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| e.into())
    }
}
impl Default for MemMigrations {
    fn default() -> Self {
        Self::new()
    }
}
impl Migrations for MemMigrations {
    type M = MemMigration;
    fn get_migration(&self, name: &str) -> Option<Self::M> {
        self.migrations.get(name).map(|m| MemMigration::clone(m))
    }
    fn latest(&self) -> Option<Self::M> {
        match &self.latest {
            None => None,
            Some(name) => self.get_migration(name),
        }
    }
}

impl MigrationsMut for MemMigrations {
    fn current(&mut self) -> &mut Self::M {
        &mut self.current
    }

    fn new_migration(&self, name: &str) -> Self::M {
        MemMigration::new(name.to_string())
    }
    fn add_migration(&mut self, m: Self::M) -> Result<()> {
        let new_latest = match &self.latest {
            None => true,
            Some(latest_name) => match m.migration_from()? {
                None => false,
                Some(name) => name.as_ref() == latest_name.as_str(),
            },
        };
        if new_latest {
            self.latest = Some(m.name().to_string());
        }

        self.migrations.insert(m.name().to_string(), m);
        Ok(())
    }
}