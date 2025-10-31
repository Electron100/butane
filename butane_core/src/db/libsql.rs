//! LibSQL database backend (remote sqld server)
//!
//! LibSQL is a fork of SQLite with additional features for remote connections.
//! This backend connects to a remote sqld (libsql-server) instance using HTTP.

use std::borrow::Cow;
use std::fmt::{Debug, Write};

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::naive::{NaiveDate, NaiveDateTime};

use super::connmethods::VecRows;
use super::helper;
use crate::db::{
    Backend, BackendConnectionAsync as BackendConnection, BackendRow,
    BackendTransactionAsync as BackendTransaction, Column, ConnectionAsync,
    ConnectionMethodsAsync as ConnectionMethods, RawQueryResult, TransactionAsync as Transaction,
};
use crate::migrations::adb::ARef;
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Order};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

#[cfg(feature = "datetime")]
const LIBSQL_DT_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

#[cfg(feature = "datetime")]
const LIBSQL_DATE_FORMAT: &str = "%Y-%m-%d";

/// Backend name identifier for LibSQL.
pub const BACKEND_NAME: &str = "libsql";

/// Row ID column name used by LibSQL (same as SQLite).
pub const ROW_ID_COLUMN_NAME: &str = "rowid";

// Trait similar to PgConnectionLike for sharing behavior between Connection and Transaction
trait LibsqlConnectionLike {
    fn conn(&self) -> Result<&libsql::Connection>;
}

impl LibsqlConnectionLike for LibsqlConnection {
    fn conn(&self) -> Result<&libsql::Connection> {
        Ok(&self.conn)
    }
}

/// LibSQL backend for remote sqld connections.
///
/// Provides an async-first database backend using LibSQL for remote connections to sqld servers.
/// LibSQL is SQLite-compatible and designed for modern async I/O operations.
///
/// # Features
///
/// - **Async-only**: All operations are asynchronous
/// - **SQLite-compatible**: Uses the same SQL dialect as SQLite
/// - **Remote connections**: Connects to sqld (libsql-server) via HTTP
/// - **Foreign key support**: Enforces foreign key constraints by default
/// - **Subquery transformation**: Automatically handles subqueries in WHERE clauses
///
/// # Limitations
///
/// - No synchronous operations (use SQLite backend if sync is required)
/// - Subqueries in WHERE clauses are transformed into multiple queries
/// - Table renames within transactions have limitations (see documentation)
/// - Requires a remote sqld server (use LibSQL backend for local/in-memory databases)
///
/// # Example
///
/// ```no_run
/// use butane_core::db::{Backend, libsql::LibsqlBackend};
///
/// # async fn example() -> Result<(), butane_core::Error> {
/// let backend = LibsqlBackend::new();
/// let mut conn = backend.connect_async("http://127.0.0.1:8080").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone)]
pub struct LibsqlBackend;
impl LibsqlBackend {
    /// Create a LibSQL backend instance.
    ///
    /// # Example
    ///
    /// ```
    /// use butane_core::db::libsql::LibsqlBackend;
    ///
    /// let backend = LibsqlBackend::new();
    /// ```
    pub fn new() -> LibsqlBackend {
        LibsqlBackend {}
    }

    async fn connect(&self, url: &str) -> Result<LibsqlConnection> {
        let connection = LibsqlConnection::open(url).await?;
        Ok(connection)
    }
}

#[async_trait]
impl Backend for LibsqlBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn row_id_column(&self) -> Option<&'static str> {
        Some(ROW_ID_COLUMN_NAME)
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        // LibSQL is SQLite-compatible, so we can reuse SQLite's migration logic
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

    async fn connect_async(&self, url: &str) -> Result<ConnectionAsync> {
        let conn = self.connect(url).await?;
        Ok(ConnectionAsync {
            conn: Box::new(conn),
        })
    }

    fn connect(&self, _url: &str) -> Result<super::Connection> {
        // LibSQL remote backend is async-only, so sync connect is not supported
        Err(Error::Internal(
            "LibSQL backend only supports async operations. Use connect_async instead.".to_string(),
        ))
    }
}

/// LibSQL database connection to remote sqld server.
pub struct LibsqlConnection {
    conn: libsql::Connection,
}

impl std::fmt::Debug for LibsqlConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibsqlConnection").finish_non_exhaustive()
    }
}

impl LibsqlConnection {
    async fn open(url: impl AsRef<str>) -> Result<Self> {
        let url_str = url.as_ref();

        // Convert libsql:// and libsql+http:// schemes to http:// for the libsql crate
        let http_url = if url_str.starts_with("libsql+http://") {
            url_str.replace("libsql+http://", "http://")
        } else if url_str.starts_with("libsql://") {
            url_str.replace("libsql://", "http://")
        } else if url_str.starts_with("http://") || url_str.starts_with("https://") {
            url_str.to_string()
        } else {
            // Assume it's a host:port without scheme
            format!("http://{}", url_str)
        };

        let db = libsql::Builder::new_remote(http_url, "".to_string())
            .build()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let conn = db.connect().map_err(|e| Error::Internal(e.to_string()))?;

        // Enable foreign key constraints using the SQL command
        conn.execute("PRAGMA foreign_keys = 1", ())
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(LibsqlConnection { conn })
    }

    /// Transform Subquery expressions into In expressions by executing the subquery first.
    /// This is necessary because LibSQL doesn't support subqueries in WHERE clauses.
    async fn transform_subqueries(&self, expr: BoolExpr) -> Result<BoolExpr> {
        use crate::query::BoolExpr::*;

        match expr {
            Subquery {
                col,
                tbl2,
                tbl2_col,
                expr: inner_expr,
            } => {
                self.execute_simple_subquery(col, tbl2, tbl2_col, inner_expr)
                    .await
            }
            SubqueryJoin {
                col,
                tbl2,
                col2,
                joins,
                expr: inner_expr,
            } => {
                self.execute_join_subquery(col, tbl2, col2, joins, inner_expr)
                    .await
            }
            And(a, b) => self.transform_binary_expr(a, b, And).await,
            Or(a, b) => self.transform_binary_expr(a, b, Or).await,
            Not(a) => {
                let a = Box::pin(self.transform_subqueries(*a)).await?;
                Ok(Not(Box::new(a)))
            }
            AllOf(exprs) => self.transform_all_of(exprs).await,
            // All other expressions pass through unchanged
            other => Ok(other),
        }
    }

    /// Execute a simple subquery and return the results as an In expression.
    async fn execute_simple_subquery(
        &self,
        col: &'static str,
        tbl2: std::borrow::Cow<'static, str>,
        tbl2_col: &'static str,
        inner_expr: Box<BoolExpr>,
    ) -> Result<BoolExpr> {
        let mut sql = format!(
            "SELECT DISTINCT {} FROM {} WHERE ",
            helper::quote_reserved_word(tbl2_col),
            helper::quote_reserved_word(&tbl2)
        );
        let mut values: Vec<SqlVal> = Vec::new();
        sql_for_expr(
            query::Expr::Condition(inner_expr),
            &mut values,
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );

        let result_values = self.execute_subquery_and_collect(&sql, values).await?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a subquery with joins and return the results as an In expression.
    async fn execute_join_subquery(
        &self,
        col: &'static str,
        tbl2: std::borrow::Cow<'static, str>,
        col2: query::Column,
        joins: Vec<query::Join>,
        inner_expr: Box<BoolExpr>,
    ) -> Result<BoolExpr> {
        let mut sql = String::new();
        write!(&mut sql, "SELECT DISTINCT ").unwrap();
        helper::sql_column(col2, &mut sql);
        write!(&mut sql, " FROM {} ", helper::quote_reserved_word(&tbl2)).unwrap();
        helper::sql_joins(joins, &mut sql);
        write!(&mut sql, " WHERE ").unwrap();

        let mut values: Vec<SqlVal> = Vec::new();
        sql_for_expr(
            query::Expr::Condition(inner_expr),
            &mut values,
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );

        let result_values = self.execute_subquery_and_collect(&sql, values).await?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a SQL query and collect the first column values.
    async fn execute_subquery_and_collect(
        &self,
        sql: &str,
        values: Vec<SqlVal>,
    ) -> Result<Vec<SqlVal>> {
        let params: Vec<libsql::Value> = values
            .iter()
            .map(|v| sqlval_to_libsql(&v.as_ref()))
            .collect();

        let mut rows = self
            .conn()?
            .query(sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let mut result_values = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
        {
            let val = row
                .get_value(0)
                .map_err(|e| Error::Internal(e.to_string()))?;
            result_values.push(libsql_value_to_sqlval_untyped(&val)?);
        }

        Ok(result_values)
    }

    /// Transform subqueries in both sides of a binary boolean expression.
    async fn transform_binary_expr(
        &self,
        a: Box<BoolExpr>,
        b: Box<BoolExpr>,
        constructor: fn(Box<BoolExpr>, Box<BoolExpr>) -> BoolExpr,
    ) -> Result<BoolExpr> {
        let a = Box::pin(self.transform_subqueries(*a)).await?;
        let b = Box::pin(self.transform_subqueries(*b)).await?;
        Ok(constructor(Box::new(a), Box::new(b)))
    }

    /// Transform all expressions in an AllOf list.
    async fn transform_all_of(&self, exprs: Vec<BoolExpr>) -> Result<BoolExpr> {
        let mut transformed = Vec::new();
        for e in exprs {
            transformed.push(Box::pin(self.transform_subqueries(e)).await?);
        }
        Ok(BoolExpr::AllOf(transformed))
    }
}

#[async_trait]
impl ConnectionMethods for LibsqlConnection {
    async fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {sql}");
        }
        // LibSQL doesn't have execute_batch, so we need to execute statements one by one
        // Split on semicolons and execute each statement separately
        for statement in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            self.conn()?
                .execute(statement, ())
                .await
                .map_err(|e| Error::Internal(e.to_string()))?;
        }
        Ok(())
    }

    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        // Transform Subquery expressions since LibSQL doesn't support them
        let expr = if let Some(expr) = expr {
            Some(self.transform_subqueries(expr).await?)
        } else {
            None
        };

        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();
        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut LibSQLPlaceholderSource::new(),
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
                // LibSQL/SQLite only supports offset in conjunction with
                // limit, so add a max limit if we don't have one already.
                helper::sql_limit(i32::MAX, &mut sqlquery)
            }
            helper::sql_offset(offset, &mut sqlquery)
        }

        debug!("query sql {sqlquery}");
        #[cfg(feature = "debug")]
        debug!("values {values:?}");

        let params: Vec<libsql::Value> = values
            .iter()
            .map(|v| sqlval_to_libsql(&v.as_ref()))
            .collect();
        let mut rows = self
            .conn()?
            .query(&sqlquery, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Collect all rows into a Vec to avoid lifetime issues
        let mut vec_rows = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
        {
            // Convert turso row to VecRow
            let mut values = Vec::new();
            for (i, col) in columns.iter().enumerate() {
                let val = row
                    .get_value(i as i32)
                    .map_err(|e| Error::Internal(e.to_string()))?;
                values.push(libsql_value_to_sqlval_typed(&val, col.ty())?);
            }
            vec_rows.push(LibSQLRow { values });
        }

        Ok(Box::new(VecRows::new(vec_rows)))
    }

    async fn insert_returning_pk(
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
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );
        // Add RETURNING clause to get the primary key value
        sql.push_str(&format!(
            " RETURNING {}",
            helper::quote_reserved_word(pkcol.name())
        ));

        if cfg!(feature = "log") {
            debug!("insert sql {sql}");
            #[cfg(feature = "debug")]
            debug!("values {values:?}");
        }

        let params: Vec<libsql::Value> = values.iter().map(|v| sqlval_to_libsql(v)).collect();

        let mut rows = self
            .conn()?
            .query(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
        {
            let val = row
                .get_value(0)
                .map_err(|e| Error::Internal(e.to_string()))?;
            // Use the pkcol type to ensure correct SqlVal type
            Ok(libsql_value_to_sqlval_typed(&val, pkcol.ty())?)
        } else {
            Err(Error::Internal(
                "Failed to retrieve inserted primary key".to_string(),
            ))
        }
    }

    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<libsql::Value> = values.iter().map(|v| sqlval_to_libsql(v)).collect();
        self.conn()?
            .execute(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(())
    }

    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        sql_insert_or_update(table, columns, pkcol, &mut sql);
        let params: Vec<libsql::Value> = values.iter().map(|v| sqlval_to_libsql(v)).collect();
        self.conn()?
            .execute(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(())
    }

    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_update_with_placeholders(
            table,
            pkcol,
            columns,
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );
        let mut params: Vec<libsql::Value> = values.iter().map(|v| sqlval_to_libsql(v)).collect();
        params.push(sqlval_to_libsql(&pk));
        self.conn()?
            .execute(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = ?",
            helper::quote_reserved_word(table),
            helper::quote_reserved_word(pkcol)
        );
        let params = vec![sqlval_to_libsql(&pk.as_ref())];
        self.conn()?
            .execute(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = format!("DELETE FROM {}", helper::quote_reserved_word(table));
        let mut values: Vec<SqlVal> = Vec::new();
        sql.write_str(" WHERE ").unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut LibSQLPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<libsql::Value> = values
            .iter()
            .map(|v| sqlval_to_libsql(&v.as_ref()))
            .collect();
        let rows_affected = self
            .conn()?
            .execute(&sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(rows_affected as usize)
    }

    async fn has_table(&self, table: &str) -> Result<bool> {
        let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
        let params = vec![libsql::Value::Text(table.to_string())];
        let mut rows = self
            .conn()?
            .query(sql, params)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(rows
            .next()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .is_some())
    }
}

#[async_trait]
impl BackendConnection for LibsqlConnection {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        // Begin transaction
        self.execute("BEGIN TRANSACTION").await?;
        let trans = Box::new(LibSQLTransaction::new(self));
        Ok(Transaction::new(trans))
    }

    fn backend(&self) -> Box<dyn Backend> {
        Box::new(LibsqlBackend {})
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn is_closed(&self) -> bool {
        false
    }
}

/// LibSQL transaction.
#[derive(Debug)]
struct LibSQLTransaction<'c> {
    conn: Option<&'c LibsqlConnection>,
    committed: bool,
}

impl<'c> LibSQLTransaction<'c> {
    fn new(conn: &'c LibsqlConnection) -> Self {
        LibSQLTransaction {
            conn: Some(conn),
            committed: false,
        }
    }

    fn get(&self) -> Result<&'c LibsqlConnection> {
        self.conn.ok_or_else(Self::already_consumed)
    }

    fn already_consumed() -> Error {
        Error::Internal("Transaction already consumed".to_string())
    }
}

impl<'c> LibsqlConnectionLike for LibSQLTransaction<'c> {
    fn conn(&self) -> Result<&libsql::Connection> {
        Ok(&self.get()?.conn)
    }
}

#[async_trait]
impl<'c> ConnectionMethods for LibSQLTransaction<'c> {
    async fn execute(&self, sql: &str) -> Result<()> {
        self.get()?.execute(sql).await
    }

    async fn query<'a>(
        &'a self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'a>> {
        self.get()?
            .query(table, columns, expr, limit, offset, sort)
            .await
    }

    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.get()?
            .insert_returning_pk(table, columns, pkcol, values)
            .await
    }

    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?.insert_only(table, columns, values).await
    }

    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?
            .insert_or_replace(table, columns, pkcol, values)
            .await
    }

    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?.update(table, pkcol, pk, columns, values).await
    }

    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.get()?.delete_where(table, expr).await
    }

    async fn has_table(&self, table: &str) -> Result<bool> {
        self.get()?.has_table(table).await
    }
}

#[async_trait]
impl<'c> BackendTransaction<'c> for LibSQLTransaction<'c> {
    async fn commit(&mut self) -> Result<()> {
        let conn = self.conn.take().ok_or_else(Self::already_consumed)?;
        conn.execute("COMMIT").await?;
        self.committed = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        let conn = self.conn.take().ok_or_else(Self::already_consumed)?;
        conn.execute("ROLLBACK").await?;
        Ok(())
    }

    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

impl<'c> Drop for LibSQLTransaction<'c> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if !self.committed {
                // If transaction was not committed or rolled back, roll it back
                // Use block_in_place to execute async rollback in a sync Drop context
                if tokio::runtime::Handle::try_current().is_ok() {
                    let conn_clone = conn.conn.clone();
                    let _ = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(async move { conn_clone.execute("ROLLBACK", ()).await })
                    });
                } else {
                    eprintln!(
                        "Warning: LibSQLTransaction dropped without commit or rollback and no tokio runtime available"
                    );
                }
            }
        }
    }
}

// Value conversion functions
fn sqlval_to_libsql(val: &SqlValRef<'_>) -> libsql::Value {
    use SqlValRef::*;
    match val {
        Bool(b) => libsql::Value::Integer(*b as i64),
        Int(i) => libsql::Value::Integer(*i as i64),
        BigInt(i) => libsql::Value::Integer(*i),
        Real(r) => libsql::Value::Real(*r),
        Text(t) => libsql::Value::Text(t.to_string()),
        Blob(b) => libsql::Value::Blob(b.to_vec()),
        #[cfg(feature = "json")]
        Json(v) => libsql::Value::Text(serde_json::to_string(v).unwrap()),
        #[cfg(feature = "datetime")]
        Date(date) => {
            let f = date.format(LIBSQL_DATE_FORMAT);
            libsql::Value::Text(f.to_string())
        }
        #[cfg(feature = "datetime")]
        Timestamp(dt) => {
            let f = dt.format(LIBSQL_DT_FORMAT);
            libsql::Value::Text(f.to_string())
        }
        Null => libsql::Value::Null,
        #[cfg(feature = "pg")]
        Custom(_) => panic!("Custom types not supported in turso"),
    }
}

fn libsql_value_to_sqlval_typed(val: &libsql::Value, ty: &SqlType) -> Result<SqlVal> {
    if matches!(val, libsql::Value::Null) {
        return Ok(SqlVal::Null);
    }

    Ok(match ty {
        SqlType::Bool => {
            let i = match val {
                libsql::Value::Integer(i) => *i,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected integer for Bool, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Bool(i != 0)
        }
        SqlType::Int => {
            let i = match val {
                libsql::Value::Integer(i) => *i,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected integer for Int, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Int(i as i32)
        }
        SqlType::BigInt => {
            let i = match val {
                libsql::Value::Integer(i) => *i,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected integer for BigInt, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::BigInt(i)
        }
        SqlType::Real => {
            let r = match val {
                libsql::Value::Real(r) => *r,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected real for Real, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Real(r)
        }
        SqlType::Text => {
            let t = match val {
                libsql::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Text, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Text(t.clone())
        }
        #[cfg(feature = "json")]
        SqlType::Json => {
            let t = match val {
                libsql::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Json, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Json(serde_json::from_str(t)?)
        }
        #[cfg(feature = "datetime")]
        SqlType::Date => {
            let t = match val {
                libsql::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Date, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Date(NaiveDate::parse_from_str(t, LIBSQL_DATE_FORMAT)?)
        }
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => {
            let t = match val {
                libsql::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Timestamp, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Timestamp(NaiveDateTime::parse_from_str(t, LIBSQL_DT_FORMAT)?)
        }
        SqlType::Blob => {
            let b = match val {
                libsql::Value::Blob(b) => b.to_vec(),
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected blob for Blob, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Blob(b)
        }
        #[cfg(feature = "pg")]
        SqlType::Custom(v) => return Err(Error::IncompatibleCustomT(v.clone(), BACKEND_NAME)),
    })
}

/// Convert a LibSQL value to SqlVal without type information.
/// This infers the SqlVal type from the LibSQL value type.
fn libsql_value_to_sqlval_untyped(val: &libsql::Value) -> Result<SqlVal> {
    Ok(match val {
        libsql::Value::Null => SqlVal::Null,
        libsql::Value::Integer(i) => SqlVal::BigInt(*i),
        libsql::Value::Real(r) => SqlVal::Real(*r),
        libsql::Value::Text(t) => SqlVal::Text(t.clone()),
        libsql::Value::Blob(b) => SqlVal::Blob(b.clone()),
    })
}

// LibSQL row wrapper that stores owned SqlVal values
struct LibSQLRow {
    values: Vec<SqlVal>,
}

impl BackendRow for LibSQLRow {
    fn get(&self, idx: usize, ty: SqlType) -> Result<SqlValRef<'_>> {
        self.values
            .get(idx)
            .ok_or_else(|| Error::Internal(format!("Column index {} out of bounds", idx)))
            .and_then(|val| {
                if val.is_compatible(&ty, true) {
                    Ok(val)
                } else {
                    Err(Error::CannotConvertSqlVal(ty.clone(), val.clone()))
                }
            })
            .map(|val| val.as_ref())
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

// Migration SQL generation (reuse SQLite logic since LibSQL is compatible)
fn sql_for_op(current: &mut ADB, op: &Operation) -> Result<String> {
    match op {
        Operation::AddTable(tbl) => Ok(create_table(tbl, false)),
        Operation::AddTableIfNotExists(tbl) => Ok(create_table(tbl, true)),
        Operation::RemoveTable(name) => Ok(drop_table(name)),
        Operation::AddColumn(tname, col) => add_column(tname, col),
        Operation::RemoveColumn(tname, colname) => remove_column(current, tname, colname),
        Operation::ChangeColumn(tname, old, new) => {
            Ok(change_column(current, tname, old, Some(new)))
        }
        Operation::AddTableConstraints(_) | Operation::RemoveTableConstraints(_) => {
            // LibSQL/SQLite doesn't support adding/removing constraints separately
            // They must be included when creating the table
            Ok(String::new())
        }
    }
}

fn create_table(table: &ATable, if_not_exists: bool) -> String {
    let mut constraints: Vec<String> = Vec::new();
    let mut defs: Vec<String> = table.columns.iter().map(define_column).collect();
    for column in &table.columns {
        if column.reference().is_some() {
            constraints.push(define_constraint(column));
        }
    }
    defs.append(&mut constraints);

    let table_name = helper::quote_reserved_word(&table.name);
    let prefix = if if_not_exists {
        format!("CREATE TABLE IF NOT EXISTS {table_name} (")
    } else {
        format!("CREATE TABLE {table_name} (")
    };

    // Format with newlines if it would be longer than 120 characters
    let single_line = format!("{}{});", prefix, defs.join(", "));
    if single_line.len() <= 120 {
        single_line
    } else {
        // Multi-line format with 4-space indentation
        let formatted_defs = defs
            .iter()
            .map(|def| format!("    {}", def))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{}\n{}\n);", prefix, formatted_defs)
    }
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

fn col_sqltype(col: &AColumn) -> Cow<'_, str> {
    match col.typeid() {
        Ok(TypeIdentifier::Ty(ty)) => Cow::Borrowed(sqltype(&ty)),
        Ok(TypeIdentifier::Name(name)) => Cow::Owned(name),
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
        SqlType::Date => "TEXT",
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => "TEXT",
        #[cfg(feature = "pg")]
        SqlType::Custom(_) => panic!("Custom types not supported by turso backend"),
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

fn remove_column(current: &mut ADB, tbl_name: &str, name: &str) -> Result<String> {
    let current_clone = current.clone();
    let table = current_clone
        .get_table(tbl_name)
        .ok_or_else(|| Error::TableNotFound(tbl_name.to_string()))?;
    let col = table
        .column(name)
        .ok_or_else(|| Error::ColumnNotFound(tbl_name.to_string(), name.to_string()))?;
    if col.reference().is_some() {
        Ok(change_column(current, tbl_name, col, None))
    } else {
        Ok(format!(
            "ALTER TABLE {} DROP COLUMN {};",
            helper::quote_reserved_word(tbl_name),
            helper::quote_reserved_word(name),
        ))
    }
}

fn copy_table(old: &ATable, new: &ATable) -> String {
    let column_names: Vec<Cow<str>> = new
        .columns
        .iter()
        .map(|col| helper::quote_reserved_word(col.name()))
        .collect();

    let column_list = column_names.join(", ");
    let single_line = format!(
        "INSERT INTO {} SELECT {} FROM {};",
        helper::quote_reserved_word(&new.name),
        column_list,
        helper::quote_reserved_word(&old.name)
    );

    // If the single line is too long, format with line breaks
    if single_line.len() <= 120 {
        single_line
    } else {
        // Multi-line format
        let formatted_columns = column_names.join(",\n    ");
        format!(
            "INSERT INTO {} SELECT\n    {}\nFROM {};",
            helper::quote_reserved_word(&new.name),
            formatted_columns,
            helper::quote_reserved_word(&old.name)
        )
    }
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
    // NOTE: LibSQL has a known limitation with ALTER TABLE RENAME operations within transactions
    // The error "table being renamed should be in schema" occurs because libSQL's schema
    // tracking doesn't properly register tables created in the same transaction.
    // For migrations that require column changes, consider skipping unmigrate for LibSQL.
    // See docs/turso-backend.md for details.
    let stmts: Vec<String> = vec![
        create_table(&new_table, false),
        copy_table(old_table, &new_table),
        drop_table(&old_table.name),
        format!(
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

/// Write SQL that performs an insert or update.
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
        write!(w, "NOTHING").unwrap();
    }
}

fn sql_for_expr(
    expr: query::Expr,
    values: &mut Vec<SqlVal>,
    placeholder_source: &mut LibSQLPlaceholderSource,
    out: &mut impl Write,
) {
    // Subqueries should already be transformed by transform_subqueries before reaching here
    // So we can just use the default helper implementation
    helper::sql_for_expr(expr, sql_for_expr, values, placeholder_source, out)
}

#[derive(Debug)]
struct LibSQLPlaceholderSource;
impl LibSQLPlaceholderSource {
    fn new() -> Self {
        LibSQLPlaceholderSource {}
    }
}
impl helper::PlaceholderSource for LibSQLPlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<'_, str> {
        // LibSQL placeholder is always a question mark (SQLite-compatible)
        Cow::Borrowed("?")
    }
}
