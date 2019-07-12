use super::helper;
use super::*;
use crate::adb::{AColumn, ATable, Operation, ADB};
use crate::query;
use crate::{Result, SqlType, SqlVal};
use hex;
use log::warn;
use rusqlite;
use std::fmt::Write;

pub struct SQLiteBackend {}
impl SQLiteBackend {
    pub fn new() -> SQLiteBackend {
        SQLiteBackend {}
    }
}
impl SQLiteBackend {
    fn connect(&self, path: &str) -> Result<SQLiteConnection> {
        SQLiteConnection::open(Path::new(path))
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

    fn connect(&self, path: &str) -> Result<Connection> {
        Ok(Connection {
            conn: Box::new(self.connect(path)?),
        })
    }
}

pub struct SQLiteConnection {
    conn: rusqlite::Connection,
}
impl SQLiteConnection {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        rusqlite::Connection::open(path)
            .map(|conn| SQLiteConnection { conn })
            .map_err(|e| e.into())
    }
}
impl BackendConnection for SQLiteConnection {
    fn execute(&self, sql: &str) -> Result<()> {
        self.conn.execute_batch(sql.as_ref())?;
        Ok(())
    }
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult> {
        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        if let Some(expr) = expr {
            sqlquery.write_str("WHERE ").unwrap();
            sql_for_expr(query::Expr::Condition(Box::new(expr)), &mut sqlquery);
        }
        if let Some(limit) = limit {
            helper::sql_limit(limit, &mut sqlquery)
        }
        let mut stmt = self.conn.prepare(&sqlquery)?;
        let rows = stmt.query_and_then(rusqlite::NO_PARAMS, |row| {
            Ok(row_from_rusqlite(row, columns)?)
        })?;
        rows.collect()
    }
}

fn row_from_rusqlite(row: &rusqlite::Row, cols: &[Column]) -> Result<Row> {
    let mut vals: Vec<SqlVal> = Vec::new();
    if cols.len() != row.column_count() {
        panic!(
            "sqlite returns columns {} doesn't match requested columns {}",
            row.column_count(),
            cols.len()
        )
    }
    vals.reserve(cols.len());
    for i in 0..cols.len() {
        let ty = cols.get(i).unwrap().ty();
        vals.push(sql_val_from_rusqlite(row.get_raw(i), ty)?);
    }
    Ok(Row::new(vals))
}

pub fn sql_for_expr<W>(expr: query::Expr, w: &mut W)
where
    W: Write,
{
    helper::sql_for_expr(expr, &sql_for_expr, w)
}

fn sql_val_from_rusqlite(val: rusqlite::types::ValueRef, ty: SqlType) -> Result<SqlVal> {
    Ok(match ty {
        SqlType::Bool => SqlVal::Bool(val.as_i64()? != 0),
        SqlType::Int => SqlVal::Int(val.as_i64()?),
        SqlType::BigInt => SqlVal::Int(val.as_i64()?),
        SqlType::Real => SqlVal::Real(val.as_f64()?),
        SqlType::Text => SqlVal::Text(val.as_str()?.to_string()),
        SqlType::Date => SqlVal::Int(val.as_i64()?),
        SqlType::Timestamp => SqlVal::Int(val.as_i64()?),
        SqlType::Blob => SqlVal::Blob(val.as_blob()?.into()),
    })
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
        .values()
        .map(define_column)
        .collect::<Vec<String>>()
        .join(",\n");
    format!("CREATE TABLE {} (\n{}\n);", table.name, coldefs)
}

fn define_column(col: &AColumn) -> String {
    let mut constraints: Vec<String> = Vec::new();
    if !col.nullable() {
        constraints.push("NOT NULL".to_string());
    }
    if col.is_pk() {
        constraints.push("PRIMARY KEY".to_string());
    }
    if let Some(defval) = col.default() {
        constraints.push(format!("DEFAULT {}", default_string(defval.clone())));
    }
    format!(
        "{} {} {}",
        &col.name(),
        col_sqltype(col),
        constraints.join(" ")
    )
}

fn default_string(d: SqlVal) -> String {
    match d {
        SqlVal::Bool(b) => {
            if b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        SqlVal::Int(i) => i.to_string(),
        SqlVal::Real(f) => f.to_string(),
        SqlVal::Text(t) => format!("'{}'", t),
        SqlVal::Blob(b) => format!("x'{}'", hex::encode(b)),
    }
}

fn col_sqltype(col: &AColumn) -> &'static str {
    match col.sqltype() {
        Ok(ty) => sqltype(ty),
        // sqlite doesn't actually require that the column type be
        // specified
        Err(_) => "",
    }
}

fn sqltype(ty: SqlType) -> &'static str {
    match ty {
        SqlType::Bool => "INTEGER",
        SqlType::Int => "INTEGER",
        SqlType::BigInt => "INTEGER",
        SqlType::Real => "REAL",
        SqlType::Text => "TEXT",
        SqlType::Date => "INTEGER",
        SqlType::Timestamp => "INTEGER",
        SqlType::Blob => "BLOB",
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
        .values()
        .map(|col| col.name())
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
            &old.name(),
            tbl_name
        );
        return "".to_string();
    }
    let old_table = table.unwrap();
    let mut new_table = old_table.clone();
    new_table.name = tmp_table_name(&new_table.name);
    match new {
        Some(col) => new_table.replace_column(col.clone()),
        None => new_table.remove_column(old.name()),
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
