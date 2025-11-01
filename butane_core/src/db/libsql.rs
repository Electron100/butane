//! Libsql database backend (remote sqld server).
//!
//! Libsql is a fork of SQLite with additional features for remote connections.
//! This backend connects to a remote sqld (libsql-server) instance using HTTP.

use std::fmt::{Debug, Write};

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::naive::{NaiveDate, NaiveDateTime};

#[cfg(feature = "async")]
use super::connmethods::VecRow;
use super::connmethods::VecRows;
use super::helper;
use super::sqlite::{
    sql_for_expr, sql_for_op, sql_insert_or_update, SQLitePlaceholderSource, HAS_TABLE_SQL,
    ROW_ID_COLUMN_NAME,
};
#[cfg(feature = "datetime")]
use super::sqlite::{SQLITE_DATE_FORMAT, SQLITE_DT_FORMAT};
use crate::db::{
    Backend, BackendConnectionAsync as BackendConnection,
    BackendTransactionAsync as BackendTransaction, Column, ConnectionAsync,
    ConnectionMethodsAsync as ConnectionMethods, RawQueryResult, TransactionAsync as Transaction,
};
use crate::migrations::adb::{Operation, ADB};
use crate::query::{BoolExpr, Order};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

/// Backend name identifier for Libsql.
pub const BACKEND_NAME: &str = "libsql";

/// Trait similar to PgConnectionLike for sharing behavior between Connection and Transaction.
trait LibsqlConnectionLike {
    fn conn(&self) -> Result<&libsql::Connection>;
}

impl LibsqlConnectionLike for LibsqlConnection {
    fn conn(&self) -> Result<&libsql::Connection> {
        Ok(&self.conn)
    }
}

/// Libsql backend for remote sqld connections.
///
/// Provides an async-first database backend using Libsql for remote connections to sqld servers.
/// Libsql is SQLite-compatible and designed for modern async I/O operations.
///
/// # Features
///
/// - **Async-only**: All operations are asynchronous
/// - **SQLite-compatible**: Uses the same SQL dialect as SQLite
/// - **Remote connections**: Connects to sqld (libsql-server) via HTTP
/// - **Foreign key support**: Enforces foreign key constraints by default
/// - **Subquery transformation**: Automatically handles subqueries in WHERE clauses
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
    /// Create a Libsql backend instance.
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
        // Libsql is SQLite-compatible, so we can reuse SQLite's migration logic
        let mut current: ADB = (*current).clone();
        let mut lines = ops
            .into_iter()
            .map(|o| {
                let sql = sql_for_op(&mut current, &o, true);
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
        // Libsql remote backend is async-only, so sync connect is not supported
        Err(Error::Internal(
            "Libsql backend only supports async operations. Use connect_async instead.".to_string(),
        ))
    }
}

/// Libsql database connection to remote sqld server.
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
    ///
    /// This is necessary because Libsql doesn't support subqueries in WHERE clauses.
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
            &mut SQLitePlaceholderSource::new(),
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
            &mut SQLitePlaceholderSource::new(),
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
        // Libsql doesn't have execute_batch, so we need to execute statements one by one
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
        // Transform Subquery expressions since Libsql doesn't support them
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
                // Libsql/SQLite only supports offset in conjunction with
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
            vec_rows.push(VecRow::new_from_values(values));
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
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        // Libsql requires the RETURNING clause to get the primary key value.
        // Using last_insert_rowid() approach (like SQLite) fails with AutoPk:
        // "Failed to retrieve last insert rowid" because the separate query
        // doesn't preserve the connection state correctly in async context.
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
            &mut SQLitePlaceholderSource::new(),
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
            &mut SQLitePlaceholderSource::new(),
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
            &mut SQLitePlaceholderSource::new(),
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
        let params = vec![libsql::Value::Text(table.to_string())];
        let mut rows = self
            .conn()?
            .query(HAS_TABLE_SQL, params)
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
        let trans = Box::new(LibsqlTransaction::new(self));
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

/// Libsql transaction.
#[derive(Debug)]
struct LibsqlTransaction<'c> {
    conn: Option<&'c LibsqlConnection>,
    committed: bool,
}

impl<'c> LibsqlTransaction<'c> {
    fn new(conn: &'c LibsqlConnection) -> Self {
        LibsqlTransaction {
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

impl<'c> LibsqlConnectionLike for LibsqlTransaction<'c> {
    fn conn(&self) -> Result<&libsql::Connection> {
        Ok(&self.get()?.conn)
    }
}

#[async_trait]
impl<'c> ConnectionMethods for LibsqlTransaction<'c> {
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
impl<'c> BackendTransaction<'c> for LibsqlTransaction<'c> {
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

impl<'c> Drop for LibsqlTransaction<'c> {
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
                        "Warning: LibsqlTransaction dropped without commit or rollback and no tokio runtime available"
                    );
                }
            }
        }
    }
}

/// Value conversion.
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
            let f = date.format(SQLITE_DATE_FORMAT);
            libsql::Value::Text(f.to_string())
        }
        #[cfg(feature = "datetime")]
        Timestamp(dt) => {
            let f = dt.format(SQLITE_DT_FORMAT);
            libsql::Value::Text(f.to_string())
        }
        Null => libsql::Value::Null,
        #[cfg(feature = "pg")]
        Custom(_) => panic!("Custom types not supported in libsql"),
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
            SqlVal::Date(NaiveDate::parse_from_str(t, SQLITE_DATE_FORMAT)?)
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
            SqlVal::Timestamp(NaiveDateTime::parse_from_str(t, SQLITE_DT_FORMAT)?)
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

/// Convert a Libsql value to [`SqlVal`] without type information.
///
/// This infers the SqlVal type from the Libsql value type.
fn libsql_value_to_sqlval_untyped(val: &libsql::Value) -> Result<SqlVal> {
    Ok(match val {
        libsql::Value::Null => SqlVal::Null,
        libsql::Value::Integer(i) => SqlVal::BigInt(*i),
        libsql::Value::Real(r) => SqlVal::Real(*r),
        libsql::Value::Text(t) => SqlVal::Text(t.clone()),
        libsql::Value::Blob(b) => SqlVal::Blob(b.clone()),
    })
}
