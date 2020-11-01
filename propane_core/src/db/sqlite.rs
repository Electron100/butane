//! SQLite database backend
use super::helper;
use super::*;
use crate::migrations::adb::{AColumn, ATable, Operation, ADB};
use crate::query;
use crate::{Result, SqlType, SqlVal};
#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;
use log::warn;
use std::fmt::Write;

#[cfg(feature = "debug")]
use exec_time::exec_time;

#[cfg(feature = "datetime")]
const SQLITE_DT_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// SQLite [Backend][crate::db::Backend] implementation.
#[derive(Default)]
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
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn create_migration_sql(&self, current: &ADB, ops: &[Operation]) -> Result<String> {
        let mut current: ADB = (*current).clone();
        Ok(ops
            .iter()
            .map(|o| sql_for_op(&mut current, o))
            .collect::<Result<Vec<String>>>()?
            .join("\n"))
    }

    fn connect(&self, path: &str) -> Result<Connection> {
        Ok(Connection {
            conn: Box::new(self.connect(path)?),
        })
    }
}

/// SQLite database connection.
pub struct SQLiteConnection {
    conn: rusqlite::Connection,
}
impl SQLiteConnection {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        rusqlite::Connection::open(path)
            .map(|conn| SQLiteConnection { conn })
            .map_err(|e| e.into())
    }

    fn wrapped_connection_methods<'a>(&'a self) -> Result<GenericConnection<'a>> {
        Ok(GenericConnection { conn: &self.conn })
    }
}
connection_method_wrapper!(SQLiteConnection);

impl BackendConnection for SQLiteConnection {
    fn transaction<'c>(&'c mut self) -> Result<Transaction<'c>> {
        let trans: rusqlite::Transaction<'_> = self.conn.transaction()?;
        let trans = Box::new(SqliteTransaction::new(trans));
        Ok(Transaction::new(trans))
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(SQLiteBackend {})
    }
    fn backend_name(&self) -> &'static str {
        "sqlite"
    }
}

// Used to provide a single implementation of ConnectionMethods that
// can be used across both SQLiteConnection and SQLiteTransaction
struct GenericConnection<'a> {
    conn: &'a rusqlite::Connection,
}

impl ConnectionMethods for GenericConnection<'_> {
    fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "debug") {
            eprintln!("execute sql {}", sql);
        }
        self.conn.execute_batch(sql.as_ref())?;
        Ok(())
    }

    #[cfg_attr(feature = "debug", exec_time)]
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult> {
        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();
        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut sqlquery,
            );
        }
        if let Some(limit) = limit {
            helper::sql_limit(limit, &mut sqlquery)
        }
        if cfg!(feature = "debug") {
            eprintln!("query sql {}", sqlquery);
        }

        let mut stmt = self.conn.prepare(&sqlquery)?;
        let rows = stmt.query_and_then(values, |row| Ok(row_from_rusqlite(row, columns)?))?;
        rows.collect()
    }
    fn insert(
        &self,
        table: &'static str,
        columns: &[Column],
        pkcol: Column,
        values: &[SqlVal],
    ) -> Result<SqlVal> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(table, columns, false, &mut sql);
        if cfg!(feature = "debug") {
            eprintln!("insert sql {}", sql);
        }
        self.conn
            .execute(&sql, &values.iter().collect::<Vec<_>>())?;
        let pk: SqlVal = self.conn.query_row_and_then(
            &format!(
                "SELECT {} FROM {} WHERE ROWID = last_insert_rowid()",
                pkcol.name(),
                table
            ),
            rusqlite::NO_PARAMS,
            |row| sql_val_from_rusqlite(row.get_raw(0), &pkcol),
        )?;
        Ok(pk)
    }
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(table, columns, true, &mut sql);
        self.conn
            .execute(&sql, &values.iter().collect::<Vec<_>>())?;
        Ok(())
    }
    fn update(
        &self,
        table: &'static str,
        pkcol: Column,
        pk: SqlVal,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_update_with_placeholders(table, pkcol, columns, &mut sql);
        let placeholder_values = [values, &[pk]].concat();
        if cfg!(feature = "debug") {
            eprintln!("update sql {}", sql);
        }
        self.conn
            .execute(&sql, &placeholder_values.iter().collect::<Vec<_>>())?;
        Ok(())
    }
    fn delete_where(&self, table: &'static str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(&mut sql, "DELETE FROM {} WHERE ", table).unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut sql,
        );
        let cnt = self.conn.execute(&sql, &values)?;
        Ok(cnt)
    }
    fn has_table(&self, table: &'static str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?;")?;
        let mut rows = stmt.query(&[table])?;
        Ok(rows.next()?.is_some())
    }
}

struct SqliteTransaction<'c> {
    trans: Option<rusqlite::Transaction<'c>>,
}
impl<'c> SqliteTransaction<'c> {
    fn new(trans: rusqlite::Transaction<'c>) -> Self {
        SqliteTransaction { trans: Some(trans) }
    }
    fn get(&self) -> Result<&rusqlite::Transaction<'c>> {
        match &self.trans {
            None => Err(Error::Internal),
            Some(trans) => Ok(trans),
        }
    }
    fn wrapped_connection_methods<'a>(&'a self) -> Result<GenericConnection<'a>> {
        Ok(GenericConnection {
            conn: self.get()?.deref(),
        })
    }
}
connection_method_wrapper!(SqliteTransaction<'_>);
impl<'c> BackendTransaction<'c> for SqliteTransaction<'c> {
    fn commit(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Error::Internal),
            Some(trans) => Ok(trans.commit()?),
        }
    }
    // Workaround for https://github.com/rust-lang/rfcs/issues/2765
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
    fn connection_methods_mut(&mut self) -> &mut dyn ConnectionMethods {
        self
    }
}

impl rusqlite::ToSql for SqlVal {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        use rusqlite::types::{ToSqlOutput::Borrowed, ToSqlOutput::Owned, Value, ValueRef};
        Ok(match self {
            SqlVal::Bool(b) => Owned(Value::Integer(if *b { 1 } else { 0 })),
            SqlVal::Int(i) => Owned(Value::Integer(*i)),
            SqlVal::Real(r) => Owned(Value::Real(*r)),
            SqlVal::Text(t) => Borrowed(ValueRef::Text(t.as_ref())),
            SqlVal::Blob(b) => Borrowed(ValueRef::Blob(&b)),
            #[cfg(feature = "datetime")]
            SqlVal::Timestamp(dt) => {
                let f = dt.format(SQLITE_DT_FORMAT);
                Owned(Value::Text(f.to_string()))
            }
            SqlVal::Null => Owned(Value::Null),
        })
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
        let col = cols.get(i).unwrap();
        vals.push(sql_val_from_rusqlite(row.get_raw(i), col)?);
    }
    Ok(Row::new(vals))
}

fn sql_for_expr<W>(expr: query::Expr, values: &mut Vec<SqlVal>, w: &mut W)
where
    W: Write,
{
    helper::sql_for_expr(expr, &sql_for_expr, values, w)
}

fn sql_val_from_rusqlite(val: rusqlite::types::ValueRef, col: &Column) -> Result<SqlVal> {
    if let rusqlite::types::ValueRef::Null = val {
        return Ok(SqlVal::Null);
    }
    let ret = || -> Result<SqlVal> {
        Ok(match col.ty() {
            SqlType::Bool => SqlVal::Bool(val.as_i64()? != 0),
            SqlType::Int => SqlVal::Int(val.as_i64()?),
            SqlType::BigInt => SqlVal::Int(val.as_i64()?),
            SqlType::Real => SqlVal::Real(val.as_f64()?),
            SqlType::Text => SqlVal::Text(val.as_str()?.to_string()),
            #[cfg(feature = "datetime")]
            SqlType::Timestamp => SqlVal::Timestamp(NaiveDateTime::parse_from_str(
                val.as_str()?,
                SQLITE_DT_FORMAT,
            )?),
            SqlType::Blob => SqlVal::Blob(val.as_blob()?.into()),
        })
    }();
    // Automatic error conversion won't have preserved the column name for any errors
    ret.map_err(|e| match e {
        Error::SqlResultTypeMismatch(_) => Error::SqlResultTypeMismatch(col.name().to_string()),
        e => e,
    })
}

fn sql_for_op(current: &mut ADB, op: &Operation) -> Result<String> {
    match op {
        Operation::AddTable(table) => Ok(create_table(&table)),
        Operation::RemoveTable(name) => Ok(drop_table(&name)),
        Operation::AddColumn(tbl, col) => add_column(&tbl, &col),
        Operation::RemoveColumn(tbl, name) => Ok(remove_column(current, &tbl, &name)),
        Operation::ChangeColumn(tbl, old, new) => Ok(change_column(current, &tbl, &old, Some(new))),
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
    if !col.nullable() {
        constraints.push("NOT NULL".to_string());
    }
    if col.is_pk() {
        constraints.push("PRIMARY KEY".to_string());
    }
    if col.is_auto() && !col.is_pk() {
        // integer primary key is automatically an alias for ROWID,
        // and we only allow auto on integer types
        constraints.push("AUTOINCREMENT".to_string());
    }
    format!(
        "{} {} {}",
        &col.name(),
        col_sqltype(col),
        constraints.join(" ")
    )
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
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => "TEXT",
        SqlType::Blob => "BLOB",
    }
}

fn drop_table(name: &str) -> String {
    format!("DROP TABLE {};", name)
}

fn add_column(tbl_name: &str, col: &AColumn) -> Result<String> {
    let default: SqlVal = helper::column_default(col)?;
    Ok(format!(
        "ALTER TABLE {} ADD COLUMN {} DEFAULT {};",
        tbl_name,
        define_column(col),
        helper::sql_literal_value(default)
    ))
}

fn remove_column(current: &mut ADB, tbl_name: &str, name: &str) -> String {
    let old = current
        .get_table(tbl_name)
        .and_then(|table| table.column(name))
        .cloned();
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
    let stmts: [&str; 4] = [
        &create_table(&new_table),
        &copy_table(&old_table, &new_table),
        &drop_table(&old_table.name),
        &format!("ALTER TABLE {} RENAME TO {};", &new_table.name, tbl_name),
    ];
    let result = stmts.join("\n");
    new_table.name = old_table.name.clone();
    current.replace_table(new_table);
    result
}
