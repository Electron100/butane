use super::*;
use crate::adb::{AColumn, ATable, AType, Operation, ADB};
use log::warn;

pub struct SQLiteBackend {}
impl SQLiteBackend {
    pub fn new() -> SQLiteBackend {
        SQLiteBackend {}
    }
}
impl Backend for SQLiteBackend {
    fn get_name(&self) -> &'static str {
        "sqlite"
    }

    fn create_migration_sql(&self, current: &ADB, ops: &[Operation]) -> String {
        let mut current: ADB = (*current).clone();
        ops.iter()
            .map(|o| sql_for_op(&mut current, o))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

fn sql_for_op(current: &mut ADB, op: &Operation) -> String {
    match op {
        Operation::AddTable(table) => create_table(&table),
        Operation::RemoveTable(name) => drop_table(&name),
        Operation::AddColumn(tbl, col) => add_column(&tbl, &col),
        Operation::RemoveColumn(tbl, name) => remove_column(current, &tbl, &name),
        Operation::ChangeColumn(tbl, old, new) => change_column(current, &tbl, &old, Some(new)),
    }
}

fn create_table(table: &ATable) -> String {
    let coldefs = table
        .columns
        .iter()
        .map(define_column)
        .collect::<Vec<String>>()
        .join(",\n");
    format!("CREATE TABLE {} (\n{}\n);", table.name, coldefs)
}

fn define_column(col: &AColumn) -> String {
    let mut constraints: Vec<String> = Vec::new();
    if !col.nullable {
        constraints.push("NOT NULL".to_string());
    }
    if col.pk {
        constraints.push("PRIMARY KEY".to_string());
    }
    if let Some(defval) = &col.default {
        constraints.push(format!("DEFAULT {}", default_string(defval.clone())));
    }
    format!(
        "{} {} {}",
        &col.name,
        sqltype(col.sqltype),
        constraints.join(" ")
    )
}

fn default_string(d: adb::DefVal) -> String {
    match d {
        adb::DefVal::Bool(b) => {
            if b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        adb::DefVal::Int(i) => i.to_string(),
        adb::DefVal::Real(f) => f.to_string(),
        adb::DefVal::Text(t) => t,
    }
}

fn sqltype(ty: AType) -> &'static str {
    match ty {
        AType::Bool => "INTEGER",
        AType::Int => "INTEGER",
        AType::BigInt => "INTEGER",
        AType::Real => "REAL",
        AType::Text => "TEXT",
        AType::Date => "INTEGER",
        AType::Timestamp => "INTEGER",
        AType::Blob => "BLOB",
    }
}

fn drop_table(name: &str) -> String {
    format!("DROP TABLE {};", name)
}

fn add_column(tbl_name: &str, col: &AColumn) -> String {
    format!("ALTER TABLE {} ADD COLUMN {}", tbl_name, define_column(col))
}

fn remove_column(current: &mut ADB, tbl_name: &str, name: &str) -> String {
    let old = current
        .get_table(tbl_name)
        .and_then(|table| table.get_column(name))
        .map(|c| c.clone());
    match old {
        Some(col) => change_column(current, tbl_name, &col, None),
        None => {
            warn!(
                "Cannot remove column {} that does not exist from table {}",
                name, tbl_name
            );
            "".to_string()
        }
    }
}

fn copy_table(old: &ATable, new: &ATable) -> String {
    let column_names = new
        .columns
        .iter()
        .map(|col| col.name.as_str())
        .collect::<Vec<&str>>()
        .join(", ");
    format!(
        "INSERT INTO {} SELECT {} FROM {};",
        &new.name, column_names, &old.name
    )
}

fn tmp_table_name(name: &str) -> String {
    format!("{}__propane_tmp", name)
}

fn change_column(
    current: &mut ADB,
    tbl_name: &str,
    old: &AColumn,
    new: Option<&AColumn>,
) -> String {
    let table = current.get_table(tbl_name);
    if table.is_none() {
        warn!(
            "Cannot alter column {} from table {} that does not exist",
            &old.name, tbl_name
        );
        return "".to_string();
    }
    let old_table = table.unwrap();
    let mut new_table = old_table.clone();
    new_table.name = tmp_table_name(&new_table.name);
    match new {
        Some(col) => new_table.replace_column(col.clone()),
        None => new_table.remove_column(&old.name),
    }
    let result = [
        "BEGIN TRANSACTION;",
        &create_table(&new_table),
        &copy_table(&old_table, &new_table),
        &drop_table(&old_table.name),
        &format!("ALTER TABLE {} RENAME TO {};", &new_table.name, tbl_name),
        "COMMIT TRANSACTION;",
    ]
    .join("\n");
    new_table.name = old_table.name.clone();
    current.replace_table(new_table);
    result
}
