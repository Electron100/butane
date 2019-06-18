use crate::SqlVal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ADB {
    pub tables: HashSet<ATable>,
}
impl ADB {
    pub fn new() -> Self {
        ADB {
            tables: HashSet::new(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AType {
    Bool,
    Int,    // 4 bytes
    BigInt, // 8 bytes
    Real,   // 8 byte float
    Text,
    Date,
    Timestamp,
    Blob,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AColumn {
    pub name: String,
    pub sqltype: AType,
    pub nullable: bool,
    pub pk: bool,
    pub default: Option<SqlVal>,
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
