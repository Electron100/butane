//! SQLite database backend
use super::helper;
use super::*;
use crate::db::connmethods::BackendRows;
use crate::debug;
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::query;
use crate::query::Order;
use crate::{Result, SqlType, SqlVal, SqlValRef};
#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;
use fallible_streaming_iterator::FallibleStreamingIterator;
use pin_project::pin_project;
use std::borrow::Cow;
use std::fmt::Write;
use std::pin::Pin;

#[cfg(feature = "datetime")]
const SQLITE_DT_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// The name of the sqlite backend.
pub const BACKEND_NAME: &str = "sqlite";

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
        BACKEND_NAME
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        let mut current: ADB = (*current).clone();
        Ok(ops
            .into_iter()
            .map(|o| {
                let sql = sql_for_op(&mut current, &o);
                current.transform_with(o);
                sql
            })
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

    // For use with connection_method_wrapper macro
    #[allow(clippy::unnecessary_wraps)]
    fn wrapped_connection_methods(&self) -> Result<&rusqlite::Connection> {
        Ok(&self.conn)
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
    fn is_closed(&self) -> bool {
        false
    }
}

impl ConnectionMethods for rusqlite::Connection {
    fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {}", sql);
        }
        self.execute_batch(sql.as_ref())?;
        Ok(())
    }

    fn query<'a, 'b, 'c: 'a>(
        &'c self,
        table: &str,
        columns: &'b [Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[Order]>,
    ) -> Result<RawQueryResult<'a>> {
        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();
        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut SQLitePlaceholderSource::new(),
                &mut sqlquery,
            );
        }

        if let Some(order) = order {
            helper::sql_order(order, &mut sqlquery)
        }

        if let Some(limit) = limit {
            helper::sql_limit(limit, &mut sqlquery)
        }

        if let Some(offset) = offset {
            helper::sql_offset(offset, &mut sqlquery)
        }

        debug!("query sql {}", sqlquery);

        let stmt = self.prepare(&sqlquery)?;
        let adapter = QueryAdapter::new(stmt, rusqlite::params_from_iter(values))?;
        Ok(Box::new(adapter))
    }
    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        if cfg!(feature = "log") {
            debug!("insert sql {}", sql);
        }
        self.execute(&sql, rusqlite::params_from_iter(values))?;
        let pk: SqlVal = self.query_row_and_then(
            &format!(
                "SELECT {} FROM {} WHERE ROWID = last_insert_rowid()",
                pkcol.name(),
                table
            ),
            [],
            |row| sql_val_from_rusqlite(row.get_ref_unwrap(0), &pkcol),
        )?;
        Ok(pk)
    }
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        if cfg!(feature = "log") {
            debug!("insert sql {}", sql);
        }
        self.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(())
    }
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        _pkcol: &Column,
        values: &[SqlValRef],
    ) -> Result<()> {
        let mut sql = String::new();
        sql_insert_or_update(table, columns, &mut sql);
        self.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(())
    }
    fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_update_with_placeholders(
            table,
            pkcol,
            columns,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        let placeholder_values = [values, &[pk]].concat();
        if cfg!(feature = "log") {
            debug!("update sql {}", sql);
        }
        self.execute(&sql, rusqlite::params_from_iter(placeholder_values))?;
        Ok(())
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(&mut sql, "DELETE FROM {} WHERE ", table).unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        let cnt = self.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(cnt)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        let mut stmt =
            self.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?;")?;
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
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans),
        }
    }
    fn wrapped_connection_methods(&self) -> Result<&rusqlite::Connection> {
        Ok(self.get()?.deref())
    }
    fn already_consumed() -> Error {
        Error::Internal("transaction has already been consumed".to_string())
    }
}
connection_method_wrapper!(SqliteTransaction<'_>);
impl<'c> BackendTransaction<'c> for SqliteTransaction<'c> {
    fn commit(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.commit()?),
        }
    }
    fn rollback(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.rollback()?),
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
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(sqlvalref_to_sqlite(&self.as_ref()))
    }
}

impl<'a> rusqlite::ToSql for SqlValRef<'a> {
    fn to_sql<'b>(&'b self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'a>> {
        Ok(sqlvalref_to_sqlite(self))
    }
}

fn sqlvalref_to_sqlite<'a, 'b>(valref: &'b SqlValRef<'a>) -> rusqlite::types::ToSqlOutput<'a> {
    use rusqlite::types::{ToSqlOutput::Borrowed, ToSqlOutput::Owned, Value, ValueRef};
    use SqlValRef::*;
    match valref {
        Bool(b) => Owned(Value::Integer(*b as i64)),
        Int(i) => Owned(Value::Integer(*i as i64)),
        BigInt(i) => Owned(Value::Integer(*i)),
        Real(r) => Owned(Value::Real(*r)),
        Text(t) => Borrowed(ValueRef::Text(t.as_bytes())),
        Blob(b) => Borrowed(ValueRef::Blob(&b)),
        #[cfg(feature = "datetime")]
        Timestamp(dt) => {
            let f = dt.format(SQLITE_DT_FORMAT);
            Owned(Value::Text(f.to_string()))
        }
        Null => Owned(Value::Null),
        Custom(_) => panic!("Custom types not supported in sqlite"),
    }
}

#[pin_project]
struct QueryAdapterInner<'a> {
    stmt: rusqlite::Statement<'a>,
    // will always be Some when the constructor has finished. We use an option only to get the
    // stmt in place before we can reference it.
    rows: Option<rusqlite::Rows<'a>>,
}

impl<'a> QueryAdapterInner<'a> {
    fn new(stmt: rusqlite::Statement<'a>, params: impl rusqlite::Params) -> Result<Pin<Box<Self>>> {
        let mut q = Box::pin(QueryAdapterInner { stmt, rows: None });
        unsafe {
            //Soundness: we pin a QueryAdapterInner value containing
            //  both the stmt and the rows referencing the statement
            //  together. It is not possible to drop/move the stmt without
            //  bringing the referencing rows along with it.
            let q_ref = Pin::get_unchecked_mut(Pin::as_mut(&mut q));
            let stmt_ref: *mut rusqlite::Statement<'a> = &mut q_ref.stmt;
            q_ref.rows = Some((&mut *stmt_ref).query(params)?)
        }
        Ok(q)
    }

    fn next<'b>(self: Pin<&'b mut Self>) -> Result<Option<&'b rusqlite::Row<'b>>> {
        let this = self.project();
        let rows: &mut rusqlite::Rows<'a> = this.rows.as_mut().unwrap();
        Ok(rows.next()?)
    }

    fn current(self: Pin<&Self>) -> Option<&rusqlite::Row> {
        let this = self.project_ref();
        this.rows.as_ref().unwrap().get()
    }
}

struct QueryAdapter<'a> {
    inner: Pin<Box<QueryAdapterInner<'a>>>,
}
impl<'a> QueryAdapter<'a> {
    fn new(stmt: rusqlite::Statement<'a>, params: impl rusqlite::Params) -> Result<Self> {
        Ok(QueryAdapter {
            inner: QueryAdapterInner::new(stmt, params)?,
        })
    }
}

impl<'a> BackendRows for QueryAdapter<'a> {
    fn next<'b>(&'b mut self) -> Result<Option<&'b (dyn BackendRow + 'b)>> {
        Ok(self
            .inner
            .as_mut()
            .next()?
            .map(|row| row as &dyn BackendRow))
    }
    fn current<'b>(&'b self) -> Option<&'b (dyn BackendRow + 'b)> {
        self.inner
            .as_ref()
            .current()
            .map(|row| row as &dyn BackendRow)
    }
}

impl BackendRow for rusqlite::Row<'_> {
    fn get(&self, idx: usize, ty: SqlType) -> Result<SqlValRef> {
        sql_valref_from_rusqlite(self.get_ref(idx)?, &ty)
    }
    fn len(&self) -> usize {
        self.column_count()
    }
}

fn sql_for_expr<W>(
    expr: query::Expr,
    values: &mut Vec<SqlVal>,
    pls: &mut SQLitePlaceholderSource,
    w: &mut W,
) where
    W: Write,
{
    helper::sql_for_expr(expr, &sql_for_expr, values, pls, w)
}

fn sql_val_from_rusqlite(val: rusqlite::types::ValueRef, col: &Column) -> Result<SqlVal> {
    sql_valref_from_rusqlite(val, col.ty()).map(|v| v.into())
}

fn sql_valref_from_rusqlite<'a>(
    val: rusqlite::types::ValueRef<'a>,
    ty: &SqlType,
) -> Result<SqlValRef<'a>> {
    if let rusqlite::types::ValueRef::Null = val {
        return Ok(SqlValRef::Null);
    }
    Ok(match ty {
        SqlType::Bool => SqlValRef::Bool(val.as_i64()? != 0),
        SqlType::Int => SqlValRef::Int(val.as_i64()? as i32),
        SqlType::BigInt => SqlValRef::BigInt(val.as_i64()?),
        SqlType::Real => SqlValRef::Real(val.as_f64()?),
        SqlType::Text => SqlValRef::Text(val.as_str()?),
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => SqlValRef::Timestamp(NaiveDateTime::parse_from_str(
            val.as_str()?,
            SQLITE_DT_FORMAT,
        )?),
        SqlType::Blob => SqlValRef::Blob(val.as_blob()?),
        SqlType::Custom(v) => {
            return Err(Error::IncompatibleCustomT(v.deref().clone(), BACKEND_NAME))
        }
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
    if col.unique() {
        constraints.push("UNIQUE".to_string());
    }
    format!(
        "{} {} {}",
        &col.name(),
        col_sqltype(col),
        constraints.join(" ")
    )
}

fn col_sqltype(col: &AColumn) -> Cow<str> {
    match col.typeid() {
        Ok(TypeIdentifier::Ty(ty)) => Cow::Borrowed(sqltype(&ty)),
        Ok(TypeIdentifier::Name(name)) => Cow::Owned(name),
        // sqlite doesn't actually require that the column type be
        // specified
        Err(_) => Cow::Borrowed(""),
    }
}

fn sqltype(ty: &SqlType) -> &'static str {
    match ty {
        SqlType::Bool => "INTEGER",
        SqlType::Int => "INTEGER",
        SqlType::BigInt => "INTEGER",
        SqlType::Real => "REAL",
        SqlType::Text => "TEXT",
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => "TEXT",
        SqlType::Blob => "BLOB",
        SqlType::Custom(_) => panic!("Custom types not supported by sqlite backend"),
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
        helper::sql_literal_value(default)?
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
            crate::warn!(
                "Cannot remove column {} that does not exist from table {}",
                name,
                tbl_name
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
    format!("{}__butane_tmp", name)
}

fn change_column(
    current: &mut ADB,
    tbl_name: &str,
    old: &AColumn,
    new: Option<&AColumn>,
) -> String {
    let table = current.get_table(tbl_name);
    if table.is_none() {
        crate::warn!(
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

pub fn sql_insert_or_update(table: &str, columns: &[Column], w: &mut impl Write) {
    write!(w, "INSERT OR REPLACE ").unwrap();
    write!(w, "INTO {} (", table).unwrap();
    helper::list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold("", |sep, _| {
        write!(w, "{}?", sep).unwrap();
        ", "
    });
    write!(w, ")").unwrap();
}

struct SQLitePlaceholderSource {}
impl SQLitePlaceholderSource {
    fn new() -> Self {
        SQLitePlaceholderSource {}
    }
}
impl helper::PlaceholderSource for SQLitePlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<str> {
        // sqlite placeholder is always a question mark.
        Cow::Borrowed("?")
    }
}
