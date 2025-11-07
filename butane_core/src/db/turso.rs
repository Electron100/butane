//! Turso database backend
//!
//! Turso is an in-process SQL database written in Rust, compatible with SQLite.
//! This backend leverages the same migration SQL and query structure as SQLite.

use std::borrow::Cow;
use std::fmt::{Debug, Write};
use std::num::{NonZero, NonZeroUsize};
use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::naive::{NaiveDate, NaiveDateTime};
use turso_core::{self, StepResult};

use super::connmethods::VecRows;
use super::helper;
// Import shared constants and helpers from sqlite module since Turso is SQLite-compatible
use super::sqlite::{
    add_column as sqlite_add_column, change_column as sqlite_change_column,
    col_sqltype as sqlite_col_sqltype, define_constraint as sqlite_define_constraint,
    drop_table as sqlite_drop_table, remove_column as sqlite_remove_column,
    sql_for_expr as sqlite_sql_for_expr, sql_insert_or_update as sqlite_sql_insert_or_update,
    sqltype as sqlite_sqltype, SQLitePlaceholderSource, ROW_ID_COLUMN_NAME,
};
#[cfg(feature = "datetime")]
use super::sqlite::{SQLITE_DATE_FORMAT, SQLITE_DT_FORMAT};
#[cfg(feature = "async-adapter")]
use crate::db::ConnectionAsync;
use crate::db::{
    Backend, BackendConnection, BackendRow, BackendTransaction, Column, ConnectionMethods,
    RawQueryResult, Transaction,
};
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Order};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

/// Backend name identifier for Turso.
pub const BACKEND_NAME: &str = "turso";

// Trait similar to PgConnectionLike for sharing behavior between Connection and Transaction
trait TursoConnectionLike {
    fn conn(&self) -> Result<&Arc<turso_core::Connection>>;
}

impl TursoConnectionLike for TursoConnection {
    fn conn(&self) -> Result<&Arc<turso_core::Connection>> {
        Ok(&self.conn)
    }
}

/// Turso backend.
///
/// Provides an async-first database backend using Turso/libSQL, which is SQLite-compatible.
/// Turso is written in Rust and designed for modern async I/O operations.
///
/// # Features
///
/// - **Async-only**: All operations are asynchronous
/// - **SQLite-compatible**: Uses the same SQL dialect as SQLite
/// - **Memory and file-based**: Supports both `:memory:` and file-based databases
/// - **Foreign key support**: Enforces foreign key constraints by default
/// - **Subquery transformation**: Automatically handles subqueries in WHERE clauses
///
/// # Limitations
///
/// - No synchronous operations (use SQLite backend if sync is required)
/// - Subqueries in WHERE clauses are transformed into multiple queries
/// - Table renames within transactions have limitations (see documentation)
///
/// # Example
///
/// ```no_run
/// use butane_core::db::{Backend, turso::TursoBackend};
///
/// # async fn example() -> Result<(), butane_core::Error> {
/// let backend = TursoBackend::new();
/// let mut conn = backend.connect(":memory:");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone)]
pub struct TursoBackend;
impl TursoBackend {
    /// Create a Turso backend instance.
    ///
    /// # Example
    ///
    /// ```
    /// use butane_core::db::turso::TursoBackend;
    ///
    /// let backend = TursoBackend::new();
    /// ```
    pub fn new() -> TursoBackend {
        TursoBackend {}
    }
}

#[async_trait]
impl Backend for TursoBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn row_id_column(&self) -> Option<&'static str> {
        Some(ROW_ID_COLUMN_NAME)
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        // Turso is SQLite-compatible, so we can reuse SQLite's migration logic
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
        Ok(format!("{}\n", lines.join("\n")))
    }

    #[cfg(feature = "async-adapter")]
    async fn connect_async(&self, path: &str) -> Result<ConnectionAsync> {
        super::adapter::connect_async_via_sync(self, path).await
    }

    #[cfg(all(feature = "async", not(feature = "async-adapter")))]
    async fn connect_async(&self, _path: &str) -> Result<ConnectionAsync> {
        Err(Error::NoAsyncAdapter("turso"))
    }

    fn connect(&self, path: &str) -> Result<super::Connection> {
        let connection = TursoConnection::open(path)?;
        Ok(super::Connection {
            conn: Box::new(connection),
        })
    }
}

/// Turso database connection.
pub struct TursoConnection {
    conn: Arc<turso_core::Connection>,
}

impl std::fmt::Debug for TursoConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TursoConnection").finish()
    }
}

impl TursoConnection {
    fn open(path: impl AsRef<str>) -> Result<Self> {
        let path_str = path.as_ref();

        // Create the IO layer for turso_core
        let io: Arc<dyn turso_core::IO> = if path_str == ":memory:" {
            Arc::new(turso_core::MemoryIO::new())
        } else {
            Arc::new(turso_core::PlatformIO::new().map_err(|e| Error::Internal(e.to_string()))?)
        };

        // Create the database with experimental indexes enabled,
        // which is what `turso` crate does.
        let db = turso_core::Database::open_file(io, path_str, false, true)
            .map_err(|e| Error::Internal(e.to_string()))?;

        let conn = db.connect().map_err(|e| Error::Internal(e.to_string()))?;

        // Enable foreign key constraints using SQL
        let mut stmt = conn.prepare("PRAGMA foreign_keys = 1")?;
        loop {
            match stmt.step()? {
                StepResult::Done => break,
                StepResult::Row => {
                    // Just consume the row
                }
                StepResult::IO => {
                    // Handle IO case if needed
                }
                _ => {
                    // Handle other cases
                }
            }
        }

        Ok(TursoConnection { conn })
    }

    /// Transform Subquery expressions into In expressions by executing the subquery first.
    /// This is necessary because Turso/libSQL doesn't support subqueries in WHERE clauses.
    fn transform_subqueries(&self, expr: BoolExpr) -> Result<BoolExpr> {
        use crate::query::BoolExpr::*;

        match expr {
            Subquery {
                col,
                tbl2,
                tbl2_col,
                expr: inner_expr,
            } => self.execute_simple_subquery(col, tbl2, tbl2_col, inner_expr),
            SubqueryJoin {
                col: col1,
                tbl2,
                col2,
                joins,
                expr: inner_expr,
            } => self.execute_join_subquery(col1, tbl2, col2, joins, inner_expr),
            And(a, b) => self.transform_binary_expr(*a, *b, And),
            Or(a, b) => self.transform_binary_expr(*a, *b, Or),
            Not(a) => {
                let a = self.transform_subqueries(*a)?;
                Ok(Not(Box::new(a)))
            }
            AllOf(exprs) => self.transform_all_of(exprs),
            // All other expressions pass through unchanged
            other => Ok(other),
        }
    }

    /// Execute a simple subquery and return the results as an In expression.
    fn execute_simple_subquery(
        &self,
        col: &'static str,
        tbl2: Cow<'static, str>,
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

        let result_values = self.execute_subquery_and_collect(&sql, values)?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a subquery with joins and return the results as an In expression.
    fn execute_join_subquery(
        &self,
        col: &'static str,
        tbl2: Cow<'static, str>,
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

        let result_values = self.execute_subquery_and_collect(&sql, values)?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a SQL query and collect the first column values.
    fn execute_subquery_and_collect(&self, sql: &str, values: Vec<SqlVal>) -> Result<Vec<SqlVal>> {
        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(&val.as_ref());
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        let mut result_values = Vec::new();

        // Execute and collect results
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Row => {
                    let val = stmt.row().unwrap().get_values().next().unwrap();
                    // Convert turso_core value back to SqlVal - we'll need the column type
                    // For now, we'll use a generic conversion
                    match turso_value_to_sqlval(val) {
                        Ok(sql_val) => result_values.push(sql_val),
                        Err(_) => {
                            return Err(Error::Internal("Failed to convert value".to_string()))
                        }
                    }
                }
                StepResult::Done => break,
                StepResult::IO => {
                    // Handle async IO - this might need special handling
                    continue;
                }
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
            }
        }

        Ok(result_values)
    }

    /// Transform subqueries in both sides of a binary boolean expression.
    fn transform_binary_expr(
        &self,
        a: BoolExpr,
        b: BoolExpr,
        constructor: fn(Box<BoolExpr>, Box<BoolExpr>) -> BoolExpr,
    ) -> Result<BoolExpr> {
        let a = self.transform_subqueries(a)?;
        let b = self.transform_subqueries(b)?;
        Ok(constructor(Box::new(a), Box::new(b)))
    }

    /// Transform all expressions in an AllOf list.
    fn transform_all_of(&self, exprs: Vec<BoolExpr>) -> Result<BoolExpr> {
        let mut transformed = Vec::new();
        for e in exprs {
            transformed.push(self.transform_subqueries(e)?);
        }
        Ok(BoolExpr::AllOf(transformed))
    }
}

impl ConnectionMethods for TursoConnection {
    fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {sql}");
        }
        // Turso doesn't have execute_batch, so we need to execute statements one by one
        // Split on semicolons and execute each statement separately
        for statement in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let mut stmt = self.conn()?.prepare(statement)?;
            loop {
                match stmt.step()? {
                    StepResult::Done => break,
                    StepResult::Row => {
                        // Just consume the row for execute
                    }
                    StepResult::IO => {
                        // Handle IO case if needed
                    }
                    _ => {
                        // Handle other cases
                    }
                }
            }
        }
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
        // Transform Subquery expressions since Turso doesn't support them
        let expr = if let Some(expr) = expr {
            Some(self.transform_subqueries(expr)?)
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
                // Turso/SQLite only supports offset in conjunction with
                // limit, so add a max limit if we don't have one already.
                helper::sql_limit(i32::MAX, &mut sqlquery)
            }
            helper::sql_offset(offset, &mut sqlquery)
        }

        debug!("query sql {sqlquery}");
        #[cfg(feature = "debug")]
        debug!("values {values:?}");

        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sqlquery)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(&val.as_ref());
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute and collect rows
        let mut vec_rows = Vec::new();
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Row => {
                    // Convert turso row to VecRow
                    let mut row_values = Vec::new();
                    let row_data: Vec<turso_core::Value> =
                        stmt.row().unwrap().get_values().cloned().collect();
                    for (i, col) in columns.iter().enumerate() {
                        if let Some(val) = row_data.get(i) {
                            let sql_val = turso_value_to_sqlval_typed(val, col.ty())?;
                            row_values.push(sql_val);
                        } else {
                            return Err(Error::Internal(format!("Missing column {} in row", i)));
                        }
                    }
                    vec_rows.push(row_values);
                }
                StepResult::Done => break,
                StepResult::IO => {
                    // Handle async IO if needed
                    continue;
                }
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
            }
        }

        // Convert to the expected format
        let turso_rows: Vec<_> = vec_rows
            .into_iter()
            .map(|values| TursoRow { values })
            .collect();
        Ok(Box::new(VecRows::new(turso_rows)))
    }

    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        _pkcol: &Column,
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
            debug!("insert sql {sql}");
            #[cfg(feature = "debug")]
            debug!("values {values:?}");
        }

        // Prepare and execute the insert statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(val);
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in insert".to_string()));
                }
            }
        }

        // Get the last inserted rowid
        let mut rowid_stmt = self
            .conn()?
            .prepare("SELECT last_insert_rowid()")
            .map_err(|e| Error::Internal(e.to_string()))?;

        match rowid_stmt
            .step()
            .map_err(|e| Error::Internal(e.to_string()))?
        {
            StepResult::Row => {
                let val = rowid_stmt.row().unwrap().get_values().next().unwrap();
                turso_value_to_sqlval(val)
            }
            _ => Err(Error::Internal(
                "Failed to get last insert rowid".to_string(),
            )),
        }
    }

    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(val);
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in insert".to_string()));
                }
            }
        }

        Ok(())
    }

    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        sql_insert_or_update(table, columns, pkcol, &mut sql);

        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(val);
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal(
                        "Unexpected row in insert_or_replace".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn update(
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
        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters (values + pk)
        let mut params: Vec<turso_core::Value> =
            values.iter().map(|v| sqlval_to_turso(v)).collect();
        params.push(sqlval_to_turso(&pk));
        for (i, val) in params.iter().enumerate() {
            stmt.bind_at(NonZero::new(i + 1).unwrap(), val.clone());
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in update".to_string()));
                }
            }
        }

        Ok(())
    }

    fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = ?",
            helper::quote_reserved_word(table),
            helper::quote_reserved_word(pkcol)
        );

        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind the primary key parameter
        let pk_val = sqlval_to_turso(&pk.as_ref());
        stmt.bind_at(NonZero::new(1).unwrap(), pk_val);

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in delete".to_string()));
                }
            }
        }

        Ok(())
    }

    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = format!("DELETE FROM {}", helper::quote_reserved_word(table));
        let mut values: Vec<SqlVal> = Vec::new();
        sql.write_str(" WHERE ").unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut SQLitePlaceholderSource::new(),
            &mut sql,
        );
        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(&val.as_ref());
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement and count affected rows
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                StepResult::Done => break,
                StepResult::IO => continue,
                StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                StepResult::Row => {
                    return Err(Error::Internal(
                        "Unexpected row in delete_where".to_string(),
                    ));
                }
            }
        }

        // Note: turso_core doesn't provide rows_affected directly like the old API
        // For now, we'll return a placeholder. A proper implementation would need
        // to track changes differently
        let rows_affected = stmt.n_change() as usize;
        Ok(rows_affected)
    }

    fn has_table(&self, table: &str) -> Result<bool> {
        let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
        let mut stmt = self.conn()?.prepare(sql)?;
        stmt.bind_at(
            NonZeroUsize::new(1).unwrap(),
            turso_core::Value::Text(table.to_string().into()),
        );

        loop {
            match stmt.step()? {
                StepResult::Done => return Ok(false),
                StepResult::Row => {
                    return Ok(true); // Found a row, table exists
                }
                StepResult::IO => {
                    // Handle IO case if needed
                }
                _ => {
                    // Handle other cases
                }
            }
        }
    }
}

impl BackendConnection for TursoConnection {
    fn transaction(&mut self) -> Result<Transaction<'_>> {
        // Begin transaction
        self.execute("BEGIN TRANSACTION")?;
        let trans = Box::new(TursoTransaction::new(self));
        Ok(Transaction::new(trans))
    }

    fn backend(&self) -> Box<dyn Backend> {
        Box::new(TursoBackend {})
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn is_closed(&self) -> bool {
        false
    }
}

/// Turso transaction.
#[derive(Debug)]
struct TursoTransaction<'c> {
    conn: Option<&'c TursoConnection>,
    committed: bool,
}

impl<'c> TursoTransaction<'c> {
    fn new(conn: &'c TursoConnection) -> Self {
        TursoTransaction {
            conn: Some(conn),
            committed: false,
        }
    }

    fn get(&self) -> Result<&'c TursoConnection> {
        self.conn.ok_or_else(Self::already_consumed)
    }

    fn already_consumed() -> Error {
        Error::Internal("Transaction already consumed".to_string())
    }
}

impl<'c> TursoConnectionLike for TursoTransaction<'c> {
    fn conn(&self) -> Result<&Arc<turso_core::Connection>> {
        Ok(&self.get()?.conn)
    }
}

impl<'c> ConnectionMethods for TursoTransaction<'c> {
    fn execute(&self, sql: &str) -> Result<()> {
        self.get()?.execute(sql)
    }

    fn query<'a>(
        &'a self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'a>> {
        self.get()?.query(table, columns, expr, limit, offset, sort)
    }

    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.get()?
            .insert_returning_pk(table, columns, pkcol, values)
    }

    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        self.get()?.insert_only(table, columns, values)
    }

    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?.insert_or_replace(table, columns, pkcol, values)
    }

    fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?.update(table, pkcol, pk, columns, values)
    }

    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.get()?.delete_where(table, expr)
    }

    fn has_table(&self, table: &str) -> Result<bool> {
        self.get()?.has_table(table)
    }
}

impl<'c> BackendTransaction<'c> for TursoTransaction<'c> {
    fn commit(&mut self) -> Result<()> {
        let conn = self.conn.take().ok_or_else(Self::already_consumed)?;
        conn.execute("COMMIT")?;
        self.committed = true;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        let conn = self.conn.take().ok_or_else(Self::already_consumed)?;
        conn.execute("ROLLBACK")?;
        Ok(())
    }

    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

impl<'c> Drop for TursoTransaction<'c> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if !self.committed {
                // If transaction was not committed or rolled back, roll it back
                // Since we're synchronous now, just execute rollback directly
                let _ = conn.execute("ROLLBACK");
            }
        }
    }
}

// Value conversion functions
fn sqlval_to_turso(val: &SqlValRef<'_>) -> turso_core::Value {
    use SqlValRef::*;
    match val {
        Bool(b) => turso_core::Value::Integer(*b as i64),
        Int(i) => turso_core::Value::Integer(*i as i64),
        BigInt(i) => turso_core::Value::Integer(*i),
        Real(r) => turso_core::Value::Float(*r),
        Text(t) => turso_core::Value::Text(t.to_string().into()),
        Blob(b) => turso_core::Value::Blob(b.to_vec()),
        #[cfg(feature = "json")]
        Json(v) => turso_core::Value::Text(serde_json::to_string(v).unwrap().into()),
        #[cfg(feature = "datetime")]
        Date(date) => {
            let f = date.format(SQLITE_DATE_FORMAT);
            turso_core::Value::Text(f.to_string().into())
        }
        #[cfg(feature = "datetime")]
        Timestamp(dt) => {
            let f = dt.format(SQLITE_DT_FORMAT);
            turso_core::Value::Text(f.to_string().into())
        }
        Null => turso_core::Value::Null,
        #[cfg(feature = "pg")]
        Custom(_) => panic!("Custom types not supported in turso"),
    }
}

/// Convert a Turso value to SqlVal without type information.
///
/// This infers the SqlVal type from the Turso value type.
fn turso_value_to_sqlval(val: &turso_core::Value) -> Result<SqlVal> {
    match val {
        turso_core::Value::Null => Ok(SqlVal::Null),
        turso_core::Value::Integer(i) => Ok(SqlVal::BigInt(*i)),
        turso_core::Value::Float(r) => Ok(SqlVal::Real(*r)),
        turso_core::Value::Text(t) => Ok(SqlVal::Text(t.to_string())),
        turso_core::Value::Blob(b) => Ok(SqlVal::Blob(b.clone())),
    }
}

fn turso_value_to_sqlval_typed(val: &turso_core::Value, ty: &SqlType) -> Result<SqlVal> {
    if matches!(val, turso_core::Value::Null) {
        return Ok(SqlVal::Null);
    }

    Ok(match ty {
        SqlType::Bool => {
            let i = match val {
                turso_core::Value::Integer(i) => *i,
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
                turso_core::Value::Integer(i) => *i,
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
                turso_core::Value::Integer(i) => *i,
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
                turso_core::Value::Float(r) => *r,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected float for Real, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Real(r)
        }
        SqlType::Text => {
            let t = match val {
                turso_core::Value::Text(t) => t.to_string(),
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
                turso_core::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Json, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Json(serde_json::from_str(t.as_str())?)
        }
        #[cfg(feature = "datetime")]
        SqlType::Date => {
            let t = match val {
                turso_core::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Date, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Date(NaiveDate::parse_from_str(t.as_str(), SQLITE_DATE_FORMAT)?)
        }
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => {
            let t = match val {
                turso_core::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Timestamp, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Timestamp(NaiveDateTime::parse_from_str(t.as_str(), SQLITE_DT_FORMAT)?)
        }
        SqlType::Blob => {
            let b = match val {
                turso_core::Value::Blob(b) => b.to_vec(),
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

// Turso row wrapper that stores owned SqlVal values
struct TursoRow {
    values: Vec<SqlVal>,
}

impl BackendRow for TursoRow {
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

// Migration SQL generation (reuse SQLite logic since Turso is compatible)
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
            // Turso/SQLite doesn't support adding/removing constraints separately
            // They must be included when creating the table
            Ok(String::new())
        }
    }
}

fn create_table(table: &ATable, if_not_exists: bool) -> String {
    let mut constraints: Vec<String> = Vec::new();
    let mut defs: Vec<String> = table.columns.iter().map(define_column).collect();
    let mut index_statements: Vec<String> = Vec::new();

    for column in &table.columns {
        if column.reference().is_some() {
            constraints.push(define_constraint(column));
        }

        // Turso requires explicit indexes for UNIQUE constraints
        if column.unique() {
            let index_name = format!("{}_{}_unique_idx", table.name, column.name());
            let index_stmt = format!(
                "CREATE UNIQUE INDEX {} ON {} ({});",
                helper::quote_reserved_word(&index_name),
                helper::quote_reserved_word(&table.name),
                helper::quote_reserved_word(column.name())
            );
            index_statements.push(index_stmt);
        }

        // Turso requires explicit indexes for non-INTEGER PRIMARY KEY constraints
        if column.is_pk() {
            let sqltype = match column.typeid() {
                Ok(TypeIdentifier::Ty(ty)) => sqlite_sqltype(&ty),
                Ok(TypeIdentifier::Name(_)) => "", // Custom types
                Err(_) => "",
            };
            // Only INTEGER PRIMARY KEY is allowed inline
            if sqltype != "INTEGER" {
                let index_name = format!("{}_{}_pk_idx", table.name, column.name());
                let index_stmt = format!(
                    "CREATE UNIQUE INDEX {} ON {} ({});",
                    helper::quote_reserved_word(&index_name),
                    helper::quote_reserved_word(&table.name),
                    helper::quote_reserved_word(column.name())
                );
                index_statements.push(index_stmt);
            }
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
    let create_table_stmt = if single_line.len() <= 120 {
        single_line
    } else {
        // Multi-line format with 4-space indentation
        let formatted_defs = defs
            .iter()
            .map(|def| format!("    {}", def))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{}\n{}\n);", prefix, formatted_defs)
    };

    // Append index creation statements
    if !index_statements.is_empty() {
        format!("{}\n{}", create_table_stmt, index_statements.join("\n"))
    } else {
        create_table_stmt
    }
}

fn define_column(col: &AColumn) -> String {
    let mut constraints: Vec<String> = Vec::new();
    if !col.nullable() {
        constraints.push("NOT NULL".to_string());
    }
    // Only add PRIMARY KEY inline for INTEGER types
    // Non-INTEGER PRIMARY KEY constraints need separate indexes in Turso
    if col.is_pk() {
        let sqltype = match col.typeid() {
            Ok(TypeIdentifier::Ty(ty)) => sqlite_sqltype(&ty),
            Ok(TypeIdentifier::Name(_)) => "", // Custom types
            Err(_) => "",
        };
        if sqltype == "INTEGER" {
            constraints.push("PRIMARY KEY".to_string());
        }
    }
    if col.is_auto() && !col.is_pk() {
        constraints.push("AUTOINCREMENT".to_string());
    }
    // Note: UNIQUE constraints are not added inline for Turso
    // because Turso requires explicit indexes for UNIQUE constraints.
    // Instead, we create separate CREATE UNIQUE INDEX statements in create_table()

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
    sqlite_define_constraint(column)
}

fn col_sqltype(col: &AColumn) -> Cow<'_, str> {
    sqlite_col_sqltype(col)
}

fn drop_table(name: &str) -> String {
    sqlite_drop_table(name)
}

fn add_column(tbl_name: &str, col: &AColumn) -> Result<String> {
    sqlite_add_column(tbl_name, col)
}

fn remove_column(current: &mut ADB, tbl_name: &str, name: &str) -> Result<String> {
    sqlite_remove_column(current, tbl_name, name, false)
}

fn change_column(
    current: &mut ADB,
    tbl_name: &str,
    old: &AColumn,
    new: Option<&AColumn>,
) -> String {
    sqlite_change_column(current, tbl_name, old, new, false)
}

/// Write SQL that performs an insert or update.
pub fn sql_insert_or_update(table: &str, columns: &[Column], pkcol: &Column, w: &mut impl Write) {
    sqlite_sql_insert_or_update(table, columns, pkcol, w)
}

fn sql_for_expr(
    expr: query::Expr,
    values: &mut Vec<SqlVal>,
    placeholder_source: &mut SQLitePlaceholderSource,
    out: &mut impl Write,
) {
    sqlite_sql_for_expr(expr, values, placeholder_source, out)
}
