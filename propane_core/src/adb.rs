use crate::{Result, SqlType, SqlVal};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeResolver {
    // The types of some columns may not be known right away
    types: HashMap<String, SqlType>,
}
impl TypeResolver {
    fn new() -> Self {
        TypeResolver {
            types: HashMap::new(),
        }
    }
    fn find_type(&self, name: &str) -> Option<SqlType> {
        self.types.get(name).map(|t| *t)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ADB {
    pub tables: HashSet<ATable>,
    // The types of some columns may not be known right away
    pub types: TypeResolver,
}
impl ADB {
    pub fn new() -> Self {
        ADB {
            tables: HashSet::new(),
            types: TypeResolver::new(),
        }
    }
    pub fn get_table<'a>(&'a self, name: &str) -> Option<&'a ATable> {
        self.tables.iter().find(|t| t.name == name)
    }
    pub fn replace_table(&mut self, table: ATable) {
        self.tables.replace(table);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ATable {
    pub name: String,
    pub columns: HashSet<AColumn>,
}
impl ATable {
    pub fn get_column<'a>(&'a self, name: &str) -> Option<&'a AColumn> {
        self.columns.iter().find(|c| c.name == name)
    }
    pub fn replace_column(&mut self, col: AColumn) {
        self.columns.replace(col);
    }
    pub fn remove_column(&mut self, name: &str) {
        self.columns = self.columns.drain().filter(|c| c.name != name).collect();
    }
}
impl Hash for ATable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

// We implement Eq for purposes of HashSet. The eqivalence
// relationship we are concerned with here is same name
impl PartialEq for ATable {
    fn eq(&self, other: &ATable) -> bool {
        self.name == other.name
    }
}
impl Eq for ATable {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeferredSqlType {
    Known(SqlType),
    Deferred(String),
}
impl DeferredSqlType {
    fn resolve(&self, resolver: &TypeResolver) -> Result<SqlType> {
        match self {
            DeferredSqlType::Known(t) => Ok(*t),
            DeferredSqlType::Deferred(key) => resolver
                .find_type(&key)
                .ok_or(crate::Error::UnknownSqlType { ty: key.clone() }.into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
            DeferredSqlType::Deferred(t) => {
                Err(crate::Error::UnknownSqlType { ty: t.clone() }.into())
            }
        }
    }
    pub fn default(&self) -> &Option<SqlVal> {
        &self.default
    }
}
impl Hash for AColumn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
// We implement Eq for purposes of HashSet. The eqivalence
// relationship we are concerned with here is same name
impl PartialEq for AColumn {
    fn eq(&self, other: &AColumn) -> bool {
        self.name == other.name
    }
}
impl Eq for AColumn {}

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

pub fn diff(old: &ADB, new: &ADB) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_tables = new.tables.difference(&old.tables);
    for added in new_tables {
        ops.push(Operation::AddTable((*added).clone()));
    }
    for removed in old.tables.difference(&new.tables) {
        ops.push(Operation::RemoveTable(removed.name.clone()));
    }
    for table in new.tables.intersection(&old.tables) {
        ops.append(&mut diff_table(
            old.tables.get(table).expect("no table"),
            table,
        ));
    }
    ops
}

fn diff_table(old: &ATable, new: &ATable) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_columns = new.columns.difference(&old.columns);
    for added in new_columns {
        ops.push(Operation::AddColumn(new.name.clone(), added.clone()));
    }
    for removed in old.columns.difference(&new.columns) {
        ops.push(Operation::RemoveColumn(
            old.name.clone(),
            removed.name.clone(),
        ));
    }
    for col in new.columns.intersection(&old.columns) {
        let old_col = old.columns.get(col).expect("no columnn");
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
