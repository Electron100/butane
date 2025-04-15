//! SQLite database backend
use std::borrow::Cow;
use std::fmt::{Debug, Write};
use std::ops::Deref;
use std::path::Path;
use std::pin::Pin;
#[cfg(feature = "log")]
use std::sync::Once;

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;
use fallible_streaming_iterator::FallibleStreamingIterator;
use pin_project::pin_project;

#[cfg(feature = "async")]
use super::ConnectionAsync;
use super::{helper, Backend, BackendRow, Column, RawQueryResult};
use super::{BackendConnection, BackendTransaction, Connection, ConnectionMethods, Transaction};
use crate::db::connmethods::BackendRows;
use crate::migrations::adb::ARef;
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Order};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

#[cfg(feature = "datetime")]
const SQLITE_DT_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

/// The name of the sqlite backend.
pub const BACKEND_NAME: &str = "sqlite";

#[cfg(feature = "log")]
fn log_callback(error_code: std::ffi::c_int, message: &str) {
    match error_code {
        rusqlite::ffi::SQLITE_NOTICE => {
            #[cfg(feature = "debug")]
            log::trace!("{}", message)
        }
        rusqlite::ffi::SQLITE_OK
        | rusqlite::ffi::SQLITE_DONE
        | rusqlite::ffi::SQLITE_NOTICE_RECOVER_WAL
        | rusqlite::ffi::SQLITE_NOTICE_RECOVER_ROLLBACK => log::info!("{}", message),
        rusqlite::ffi::SQLITE_WARNING | rusqlite::ffi::SQLITE_WARNING_AUTOINDEX => {
            log::warn!("{}", message)
        }
        _ => log::error!("{error_code} {}", message),
    }
}

/// SQLite [`Backend`] implementation.
#[derive(Debug, Default, Clone)]
pub struct SQLiteBackend;
impl SQLiteBackend {
    pub fn new() -> SQLiteBackend {
        SQLiteBackend {}
    }
}
impl SQLiteBackend {
    fn connect(&self, path: &str) -> Result<SQLiteConnection> {
        let connection = SQLiteConnection::open(Path::new(path))?;
        connection.execute("PRAGMA foreign_keys = ON")?;
        Ok(connection)
    }
}

#[async_trait]
impl Backend for SQLiteBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        let mut current: ADB = (*current).clone();
        let mut lines = ops
            .into_iter()
            .map(|o| {
                let sql = sql_for_op(&mut current, &o);
                current.transform_with(o);
                sql
            })
            .collect::<Result<Vec<String>>>()?;
        lines.retain(|s| !s.is_empty());
        Ok(lines.join("\n"))
    }

    fn connect(&self, path: &str) -> Result<Connection> {
        Ok(Connection {
            conn: Box::new(self.connect(path)?),
        })
    }
    #[cfg(feature = "async-adapter")]
    async fn connect_async(&self, path: &str) -> Result<ConnectionAsync> {
        super::adapter::connect_async_via_sync(self, path).await
    }

    #[cfg(all(feature = "async", not(feature = "async-adapter")))]
    async fn connect_async(&self, _path: &str) -> Result<ConnectionAsync> {
        Err(Error::NoAsyncAdapter("sqlite"))
    }
}

/// SQLite database connection.
#[derive(Debug)]
pub struct SQLiteConnection {
    conn: rusqlite::Connection,
}
impl SQLiteConnection {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(feature = "log")]
        static INIT_SQLITE_LOGGING: Once = Once::new();

        #[cfg(feature = "log")]
        INIT_SQLITE_LOGGING.call_once(|| {
            _ = unsafe { rusqlite::trace::config_log(Some(log_callback)) };
        });

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

impl ConnectionMethods for SQLiteConnection {
    fn execute(&self, sql: &str) -> Result<()> {
        ConnectionMethods::execute(self.wrapped_connection_methods()?, sql)
    }
    fn query<'a, 'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[crate::query::Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.wrapped_connection_methods()?
            .query(table, columns, expr, limit, offset, sort)
    }
    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.wrapped_connection_methods()?
            .insert_returning_pk(table, columns, pkcol, values)
    }
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        self.wrapped_connection_methods()?
            .insert_only(table, columns, values)
    }
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.wrapped_connection_methods()?
            .insert_or_replace(table, columns, pkcol, values)
    }
    fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.wrapped_connection_methods()?
            .update(table, pkcol, pk, columns, values)
    }
    fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.wrapped_connection_methods()?.delete(table, pkcol, pk)
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.wrapped_connection_methods()?.delete_where(table, expr)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        self.wrapped_connection_methods()?.has_table(table)
    }
}

impl BackendConnection for SQLiteConnection {
    fn transaction(&mut self) -> Result<Transaction<'_>> {
        let trans: rusqlite::Transaction<'_> = self.conn.transaction()?;
        let trans = Box::new(SqliteTransaction::new(trans));
        Ok(Transaction::new(trans))
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(SQLiteBackend {})
    }
    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
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

    fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
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
            if limit.is_none() {
                // Sqlite only supports offset in conjunction with
                // limit, so add a max limit if we don't have one
                // already.
                helper::sql_limit(i32::MAX, &mut sqlquery)
            }
            helper::sql_offset(offset, &mut sqlquery)
        }

        debug!("query sql {}", sqlquery);
        #[cfg(feature = "debug")]
        debug!("values {:?}", values);

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
            #[cfg(feature = "debug")]
            debug!("values {:?}", values);
        }
        self.execute(&sql, rusqlite::params_from_iter(values))?;
        let pk: SqlVal = self.query_row_and_then(
            &format!(
                "SELECT {} FROM {} WHERE ROWID = last_insert_rowid()",
                pkcol.name(),
                table
            ),
            [],
            |row| sql_val_from_rusqlite(row.get_ref_unwrap(0), pkcol),
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
            #[cfg(feature = "debug")]
            debug!("values {:?}", values);
        }
        self.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(())
    }
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef],
    ) -> Result<()> {
        let mut sql = String::new();
        sql_insert_or_update(table, columns, pkcol, &mut sql);
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
            #[cfg(feature = "debug")]
            debug!("placeholders {:?}", placeholder_values);
        }
        self.execute(&sql, rusqlite::params_from_iter(placeholder_values))?;
        Ok(())
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(
            &mut sql,
            "DELETE FROM {} WHERE ",
            helper::quote_reserved_word(table)
        )
        .unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        if cfg!(feature = "log") {
            debug!("delete where sql {}", sql);
            #[cfg(feature = "debug")]
            debug!("placeholders {:?}", values);
        }
        let cnt = self.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(cnt)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        let mut stmt =
            self.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?;")?;
        let mut rows = stmt.query([table])?;
        Ok(rows.next()?.is_some())
    }
}

#[derive(Debug)]
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
impl ConnectionMethods for SqliteTransaction<'_> {
    fn execute(&self, sql: &str) -> Result<()> {
        ConnectionMethods::execute(self.wrapped_connection_methods()?, sql)
    }
    fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[crate::query::Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.wrapped_connection_methods()?
            .query(table, columns, expr, limit, offset, sort)
    }
    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.wrapped_connection_methods()?
            .insert_returning_pk(table, columns, pkcol, values)
    }
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        self.wrapped_connection_methods()?
            .insert_only(table, columns, values)
    }
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.wrapped_connection_methods()?
            .insert_or_replace(table, columns, pkcol, values)
    }
    fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.wrapped_connection_methods()?
            .update(table, pkcol, pk, columns, values)
    }
    fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.wrapped_connection_methods()?.delete(table, pkcol, pk)
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.wrapped_connection_methods()?.delete_where(table, expr)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        self.wrapped_connection_methods()?.has_table(table)
    }
}

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

fn sqlvalref_to_sqlite<'a>(valref: &SqlValRef<'a>) -> rusqlite::types::ToSqlOutput<'a> {
    use rusqlite::types::{ToSqlOutput::Borrowed, ToSqlOutput::Owned, Value, ValueRef};
    use SqlValRef::*;
    match valref {
        Bool(b) => Owned(Value::Integer(*b as i64)),
        Int(i) => Owned(Value::Integer(*i as i64)),
        BigInt(i) => Owned(Value::Integer(*i)),
        Real(r) => Owned(Value::Real(*r)),
        Text(t) => Borrowed(ValueRef::Text(t.as_bytes())),
        Blob(b) => Borrowed(ValueRef::Blob(b)),
        #[cfg(feature = "json")]
        Json(v) => serde_json::to_string(v)
            .map(rusqlite::types::ToSqlOutput::from)
            .unwrap(),
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
// Debug can not be derived because rusqlite::Rows doesn't implement it.
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
            q_ref.rows = Some((*stmt_ref).query(params)?)
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

// Debug can not be derived because QueryAdapterInner above doesn't implement it.
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

impl BackendRows for QueryAdapter<'_> {
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
        self.as_ref().column_count()
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
    helper::sql_for_expr(expr, sql_for_expr, values, pls, w)
}

fn sql_val_from_rusqlite(val: rusqlite::types::ValueRef, col: &Column) -> Result<SqlVal> {
    sql_valref_from_rusqlite(val, col.ty()).map(|v| v.into())
}

fn sql_valref_from_rusqlite<'a>(
    val: rusqlite::types::ValueRef<'a>,
    ty: &SqlType,
) -> Result<SqlValRef<'a>> {
    if matches!(val, rusqlite::types::ValueRef::Null) {
        return Ok(SqlValRef::Null);
    }
    Ok(match ty {
        SqlType::Bool => SqlValRef::Bool(val.as_i64()? != 0),
        SqlType::Int => SqlValRef::Int(val.as_i64()? as i32),
        SqlType::BigInt => SqlValRef::BigInt(val.as_i64()?),
        SqlType::Real => SqlValRef::Real(val.as_f64()?),
        SqlType::Text => SqlValRef::Text(val.as_str()?),
        #[cfg(feature = "json")]
        SqlType::Json => SqlValRef::Json(serde_json::from_str(val.as_str()?)?),
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => SqlValRef::Timestamp(NaiveDateTime::parse_from_str(
            val.as_str()?,
            SQLITE_DT_FORMAT,
        )?),
        SqlType::Blob => SqlValRef::Blob(val.as_blob()?),
        SqlType::Custom(v) => return Err(Error::IncompatibleCustomT(v.clone(), BACKEND_NAME)),
    })
}

fn sql_for_op(current: &mut ADB, op: &Operation) -> Result<String> {
    match op {
        Operation::AddTable(table) => Ok(create_table(table, false)),
        Operation::AddTableConstraints(_table) => Ok("".to_owned()),
        Operation::AddTableIfNotExists(table) => Ok(create_table(table, true)),
        Operation::RemoveTable(name) => Ok(drop_table(name)),
        Operation::RemoveTableConstraints(_table) => Ok("".to_owned()),
        Operation::AddColumn(tbl, col) => add_column(tbl, col),
        Operation::RemoveColumn(tbl, name) => Ok(remove_column(current, tbl, name)),
        Operation::ChangeColumn(tbl, old, new) => Ok(change_column(current, tbl, old, Some(new))),
    }
}

fn create_table(table: &ATable, allow_exists: bool) -> String {
    let coldefs = table
        .columns
        .iter()
        .map(define_column)
        .collect::<Vec<String>>()
        .join(",\n");
    let modifier = if allow_exists { "IF NOT EXISTS " } else { "" };
    let mut constraints = create_table_constraints(table);
    if !constraints.is_empty() {
        constraints = ",\n".to_owned() + &constraints;
    }
    format!(
        "CREATE TABLE {}{} (\n{}{}\n) STRICT;",
        modifier, table.name, coldefs, constraints
    )
}

fn create_table_constraints(table: &ATable) -> String {
    table
        .columns
        .iter()
        .filter(|column| column.reference().is_some())
        .map(define_constraint)
        .collect::<Vec<String>>()
        .join("\n")
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
    if constraints.is_empty() {
        format!(
            "{} {}",
            helper::quote_reserved_word(col.name()),
            col_sqltype(col),
        )
    } else {
        format!(
            "{} {} {}",
            helper::quote_reserved_word(col.name()),
            col_sqltype(col),
            constraints.join(" ")
        )
    }
}

fn define_constraint(column: &AColumn) -> String {
    let reference = column
        .reference()
        .as_ref()
        .expect("must have a references value");
    match reference {
        ARef::Literal(literal) => {
            format!(
                "FOREIGN KEY ({}) REFERENCES {}({})",
                helper::quote_reserved_word(column.name()),
                helper::quote_reserved_word(literal.table_name()),
                helper::quote_reserved_word(literal.column_name()),
            )
        }
        _ => panic!(),
    }
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
        SqlType::Blob => "BLOB",
        #[cfg(feature = "json")]
        SqlType::Json => "TEXT",
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => "TEXT",
        SqlType::Custom(_) => panic!("Custom types not supported by sqlite backend"),
    }
}

fn drop_table(name: &str) -> String {
    format!("DROP TABLE {};", helper::quote_reserved_word(name))
}

fn add_column(tbl_name: &str, col: &AColumn) -> Result<String> {
    let default: SqlVal = helper::column_default(col)?;
    Ok(format!(
        "ALTER TABLE {} ADD COLUMN {} DEFAULT {};",
        helper::quote_reserved_word(tbl_name),
        define_column(col),
        helper::sql_literal_value(&default)?
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
        .map(|col| helper::quote_reserved_word(col.name()))
        .collect::<Vec<Cow<str>>>()
        .join(", ");
    format!(
        "INSERT INTO {} SELECT {} FROM {};",
        helper::quote_reserved_word(&new.name),
        column_names,
        helper::quote_reserved_word(&old.name)
    )
}

fn tmp_table_name(name: &str) -> String {
    format!("{name}__butane_tmp")
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
        &create_table(&new_table, false),
        &copy_table(old_table, &new_table),
        &drop_table(&old_table.name),
        &format!(
            "ALTER TABLE {} RENAME TO {};",
            helper::quote_reserved_word(&new_table.name),
            helper::quote_reserved_word(tbl_name)
        ),
    ];
    let result = stmts.join("\n");
    new_table.name.clone_from(&old_table.name);
    current.replace_table(new_table);
    result
}

pub fn sql_insert_or_update(table: &str, columns: &[Column], pkcol: &Column, w: &mut impl Write) {
    write!(w, "INSERT ").unwrap();
    write!(w, "INTO {} (", helper::quote_reserved_word(table)).unwrap();
    helper::list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold("", |sep, _| {
        write!(w, "{sep}?").unwrap();
        ", "
    });
    write!(w, ")").unwrap();
    write!(w, " ON CONFLICT ({}) DO ", pkcol.name()).unwrap();
    if columns.len() > 1 {
        write!(w, "UPDATE SET (").unwrap();
        helper::list_columns(columns, w);
        write!(w, ") = (").unwrap();
        columns.iter().fold("", |sep, c| {
            write!(
                w,
                "{}excluded.{}",
                sep,
                helper::quote_reserved_word(c.name())
            )
            .unwrap();
            ", "
        });
        write!(w, ")").unwrap();
    } else {
        // If the pk is the only column and it already exists, then there's nothing to update.
        write!(w, "NOTHING").unwrap();
    }
}

#[derive(Debug)]
struct SQLitePlaceholderSource;
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
