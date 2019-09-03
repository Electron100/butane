//! Abstract representation of a database schema. If using the propane
//! CLI tool, there is no need to use this module. Even if applying
//! migrations without this tool, you are unlikely to need this module.

use crate::{Result, SqlType, SqlVal};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Key used to help resolve `DeferredSqlType`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeKey {
    /// Represents a type which is the primary key for a table with the given name
    PK(String),
}
impl std::fmt::Display for TypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            TypeKey::PK(name) => write!(f, "PK({})", name),
        }
    }
}

#[derive(Debug)]
struct TypeResolver {
    // The types of some columns may not be known right away
    types: HashMap<TypeKey, SqlType>,
}
impl TypeResolver {
    fn new() -> Self {
        TypeResolver {
            types: HashMap::new(),
        }
    }
    fn find_type(&self, key: &TypeKey) -> Option<SqlType> {
        self.types.get(key).copied()
    }
    fn insert(&mut self, key: TypeKey, ty: SqlType) -> bool {
        use std::collections::hash_map::Entry;
        let entry = self.types.entry(key);
        match entry {
            Entry::Occupied(_) => false,
            Entry::Vacant(e) => {
                e.insert(ty);
                true
            }
        }
    }
    fn insert_pk(&mut self, key: &str, ty: SqlType) -> bool {
        self.insert(TypeKey::PK(key.to_string()), ty)
    }
}

/// Abstract representation of a database schema.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ADB {
    tables: HashMap<String, ATable>,
}
impl ADB {
    pub fn new() -> Self {
        ADB {
            tables: HashMap::new(),
        }
    }
    pub fn tables(&self) -> impl Iterator<Item = &ATable> {
        self.tables.values()
    }
    pub fn get_table<'a>(&'a self, name: &str) -> Option<&'a ATable> {
        self.tables.get(name)
    }
    pub fn replace_table(&mut self, table: ATable) {
        self.tables.insert(table.name.clone(), table);
    }
    /// Fixup as many DeferredSqlType::Deferred instances as possible
    /// into DeferredSqlType::Known
    pub fn resolve_types(&mut self) -> Result<()> {
        let mut resolver = TypeResolver::new();
        let mut changed = true;
        while changed {
            changed = false;
            for table in &mut self.tables.values_mut() {
                if let Some(pk) = table.pk() {
                    let pktype = pk.sqltype();
                    if let Ok(pktype) = pktype {
                        changed = resolver.insert_pk(&table.name, pktype)
                    }

                    for col in table.columns.values_mut() {
                        col.resolve_type(&resolver);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Abstract representation of a database table schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ATable {
    pub name: String,
    pub columns: HashMap<String, AColumn>,
}
impl ATable {
    pub fn new(name: String) -> ATable {
        ATable {
            name,
            columns: HashMap::new(),
        }
    }
    pub fn add_column(&mut self, col: AColumn) {
        self.replace_column(col);
    }
    pub fn column<'a>(&'a self, name: &str) -> Option<&'a AColumn> {
        self.columns.get(name)
    }
    pub fn replace_column(&mut self, col: AColumn) {
        self.columns.insert(col.name.clone(), col);
    }
    pub fn remove_column(&mut self, name: &str) {
        self.columns.remove(name);
    }
    pub fn pk(&self) -> Option<&AColumn> {
        self.columns.values().find(|c| c.is_pk())
    }
}

/// SqlType which may not yet be known.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum DeferredSqlType {
    Known(SqlType),
    Deferred(TypeKey),
}
impl DeferredSqlType {
    fn resolve(&self, resolver: &TypeResolver) -> Result<SqlType> {
        match self {
            DeferredSqlType::Known(t) => Ok(*t),
            DeferredSqlType::Deferred(key) => {
                resolver
                    .find_type(&key)
                    .ok_or_else(|| crate::Error::UnknownSqlType {
                        ty: key.to_string(),
                    })
            }
        }
    }
}

/// Abstract representation of a database column schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AColumn {
    name: String,
    sqltype: DeferredSqlType,
    nullable: bool,
    pk: bool,
    default: Option<SqlVal>,
}
impl AColumn {
    pub fn new(
        name: impl Into<String>,
        sqltype: DeferredSqlType,
        nullable: bool,
        pk: bool,
        default: Option<SqlVal>,
    ) -> Self {
        AColumn {
            name: name.into(),
            sqltype,
            nullable,
            pk,
            default,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn nullable(&self) -> bool {
        self.nullable
    }
    pub fn is_pk(&self) -> bool {
        self.pk
    }
    pub fn sqltype(&self) -> Result<SqlType> {
        match &self.sqltype {
            DeferredSqlType::Known(t) => Ok(*t),
            DeferredSqlType::Deferred(t) => Err(crate::Error::UnknownSqlType { ty: t.to_string() }),
        }
    }
    fn resolve_type(&mut self, resolver: &'_ TypeResolver) -> Option<SqlType> {
        if let Ok(ty) = self.sqltype.resolve(resolver) {
            self.sqltype = DeferredSqlType::Known(ty);
            Some(ty)
        } else {
            None
        }
    }
    pub fn default(&self) -> &Option<SqlVal> {
        &self.default
    }
}

/// Individual operation use to apply a migration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    //TODO support renames
    //TODO support changed default
    AddTable(ATable),
    RemoveTable(String),
    AddColumn(String, AColumn),
    RemoveColumn(String, String),
    ChangeColumn(String, AColumn, AColumn),
}

/// Determine the operations necessary to move the database schema from `old` to `new`.
pub fn diff(old: &ADB, new: &ADB) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_names: HashSet<&String> = new.tables.keys().collect();
    let old_names: HashSet<&String> = old.tables.keys().collect();
    let new_tables = new_names.difference(&old_names);
    for added in new_tables {
        let added: &str = added.as_ref();
        ops.push(Operation::AddTable(
            new.tables.get(added).expect("no table").clone(),
        ));
    }
    for removed in old_names.difference(&new_names) {
        ops.push(Operation::RemoveTable(removed.to_string()));
    }
    for table in new_names.intersection(&old_names) {
        let table: &str = table.as_ref();
        ops.append(&mut diff_table(
            old.tables.get(table).expect("no table"),
            new.tables.get(table).expect("no table"),
        ));
    }
    ops
}

fn diff_table(old: &ATable, new: &ATable) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_names: HashSet<&String> = new.columns.keys().collect();
    let old_names: HashSet<&String> = old.columns.keys().collect();
    let added_names = new_names.difference(&old_names);
    for added in added_names {
        let added: &str = added.as_ref();
        ops.push(Operation::AddColumn(
            new.name.clone(),
            new.columns.get(added).unwrap().clone(),
        ));
    }
    for removed in old_names.difference(&new_names) {
        ops.push(Operation::RemoveColumn(
            old.name.clone(),
            removed.to_string(),
        ));
    }
    for colname in new_names.intersection(&old_names) {
        let colname: &str = colname.as_ref();
        let col = new.columns.get(colname).unwrap();
        let old_col = old.columns.get(colname).unwrap();
        if col == old_col {
            continue;
        }
        ops.push(Operation::ChangeColumn(
            new.name.clone(),
            old_col.clone(),
            col.clone(),
        ));
    }
    ops
}
