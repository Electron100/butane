//! Turso database backend
//!
//! Turso is an in-process SQL database written in Rust, compatible with SQLite.
//! This backend leverages the same migration SQL and query structure as SQLite.

use std::borrow::Cow;
use std::fmt::{Debug, Write};

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::naive::{NaiveDate, NaiveDateTime};

use super::connmethods::VecRows;
use super::helper;
use crate::db::{
    Backend, BackendConnection, BackendRow, BackendTransaction, Column,
    ConnectionMethods, RawQueryResult, Transaction,
};
#[cfg(feature = "async-adapter")]
use crate::db::ConnectionAsync;
use crate::migrations::adb::ARef;
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Order};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

// Import turso_core for database functionality
#[cfg(feature = "turso")]
use turso_core as turso;

#[cfg(feature = "datetime")]
const TURSO_DT_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

#[cfg(feature = "datetime")]
const TURSO_DATE_FORMAT: &str = "%Y-%m-%d";

/// Backend name identifier for Turso.
pub const BACKEND_NAME: &str = "turso";

/// Row ID column name used by Turso (same as SQLite).
pub const ROW_ID_COLUMN_NAME: &str = "rowid";

// Trait similar to PgConnectionLike for sharing behavior between Connection and Transaction
trait TursoConnectionLike {
    fn conn(&self) -> Result<&std::sync::Arc<turso::Connection>>;
}

impl TursoConnectionLike for TursoConnection {
    fn conn(&self) -> Result<&std::sync::Arc<turso::Connection>> {
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
    conn: std::sync::Arc<turso::Connection>,
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
        let io: std::sync::Arc<dyn turso::IO> = if path_str == ":memory:" {
            std::sync::Arc::new(turso::MemoryIO::new())
        } else {
            std::sync::Arc::new(
                turso::PlatformIO::new()
                    .map_err(|e| Error::Internal(e.to_string()))?
            )
        };

        // Create the turso_core database
        let db = turso::Database::open_file(io, path_str, false, false)
            .map_err(|e| Error::Internal(e.to_string()))?;

        let conn = db.connect()
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Enable foreign key constraints using SQL
        let mut stmt = conn.prepare("PRAGMA foreign_keys = 1")?;
        loop {
            match stmt.step()? {
                turso::StepResult::Done => break,
                turso::StepResult::Row => {
                    // Just consume the row
                }
                turso::StepResult::IO => {
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
            } => {
                self.execute_simple_subquery(col, tbl2, tbl2_col, inner_expr)
            }
            SubqueryJoin {
                col: col1,
                tbl2,
                col2,
                joins,
                expr: inner_expr,
            } => {
                self.execute_join_subquery(col1, tbl2, col2, joins, inner_expr)
            }
            And(a, b) => self.transform_binary_expr(a, b, And),
            Or(a, b) => self.transform_binary_expr(a, b, Or),
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
            &mut TursoPlaceholderSource::new(),
            &mut sql,
        );

        let result_values = self.execute_subquery_and_collect(&sql, values)?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a subquery with joins and return the results as an In expression.
    fn execute_join_subquery(
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
            &mut TursoPlaceholderSource::new(),
            &mut sql,
        );

        let result_values = self.execute_subquery_and_collect(&sql, values)?;
        Ok(BoolExpr::In(col, result_values))
    }

    /// Execute a SQL query and collect the first column values.
    fn execute_subquery_and_collect(
        &self,
        sql: &str,
        values: Vec<SqlVal>,
    ) -> Result<Vec<SqlVal>> {
        // Prepare the statement
        let mut stmt = self.conn()?.prepare(sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters
        for (i, val) in values.iter().enumerate() {
            let turso_val = sqlval_to_turso(&val.as_ref());
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        let mut result_values = Vec::new();

        // Execute and collect results
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Row => {
                    let val = stmt.row().unwrap().get_values().next().unwrap();
                    // Convert turso_core value back to SqlVal - we'll need the column type
                    // For now, we'll use a generic conversion
                    match turso_value_to_sqlval(&val) {
                        Ok(sql_val) => result_values.push(sql_val),
                        Err(_) => return Err(Error::Internal("Failed to convert value".to_string())),
                    }
                }
                turso::StepResult::Done => break,
                turso::StepResult::IO => {
                    // Handle async IO - this might need special handling
                    continue;
                }
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
            }
        }

        Ok(result_values)
    }

    /// Transform subqueries in both sides of a binary boolean expression.
    fn transform_binary_expr(
        &self,
        a: Box<BoolExpr>,
        b: Box<BoolExpr>,
        constructor: fn(Box<BoolExpr>, Box<BoolExpr>) -> BoolExpr,
    ) -> Result<BoolExpr> {
        let a = self.transform_subqueries(*a)?;
        let b = self.transform_subqueries(*b)?;
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
                    turso::StepResult::Done => break,
                    turso::StepResult::Row => {
                        // Just consume the row for execute
                    }
                    turso::StepResult::IO => {
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
                &mut TursoPlaceholderSource::new(),
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
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute and collect rows
        let mut vec_rows = Vec::new();
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Row => {
                    // Convert turso row to VecRow
                    let mut row_values = Vec::new();
                    let row_data: Vec<turso::Value> = stmt.row().unwrap().get_values().cloned().collect();
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
                turso::StepResult::Done => break,
                turso::StepResult::IO => {
                    // Handle async IO if needed
                    continue;
                }
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
            }
        }

        // Convert to the expected format
        let turso_rows: Vec<_> = vec_rows.into_iter().map(|values| TursoRow { values }).collect();
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
            &mut TursoPlaceholderSource::new(),
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
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in insert".to_string()));
                }
            }
        }

        // Get the last inserted rowid
        let mut rowid_stmt = self
            .conn()?
            .prepare("SELECT last_insert_rowid()")
            .map_err(|e| Error::Internal(e.to_string()))?;

        match rowid_stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
            turso::StepResult::Row => {
                let val = rowid_stmt.row().unwrap().get_values().next().unwrap();
                turso_value_to_sqlval(&val)
            }
            _ => Err(Error::Internal("Failed to get last insert rowid".to_string())),
        }
    }

    fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut TursoPlaceholderSource::new(),
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
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
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
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in insert_or_replace".to_string()));
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
            &mut TursoPlaceholderSource::new(),
            &mut sql,
        );
        // Prepare the statement
        let mut stmt = self
            .conn()?
            .prepare(&sql)
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Bind parameters (values + pk)
        let mut params: Vec<turso::Value> = values.iter().map(|v| sqlval_to_turso(v)).collect();
        params.push(sqlval_to_turso(&pk));
        for (i, val) in params.iter().enumerate() {
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), val.clone());
        }

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
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
        stmt.bind_at(std::num::NonZero::new(1).unwrap(), pk_val);

        // Execute the statement
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
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
            &mut TursoPlaceholderSource::new(),
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
            stmt.bind_at(std::num::NonZero::new(i + 1).unwrap(), turso_val);
        }

        // Execute the statement and count affected rows
        loop {
            match stmt.step().map_err(|e| Error::Internal(e.to_string()))? {
                turso::StepResult::Done => break,
                turso::StepResult::IO => continue,
                turso::StepResult::Busy => {
                    return Err(Error::Internal("Database is busy".to_string()));
                }
                turso::StepResult::Interrupt => {
                    return Err(Error::Internal("Query interrupted".to_string()));
                }
                turso::StepResult::Row => {
                    return Err(Error::Internal("Unexpected row in delete_where".to_string()));
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
        stmt.bind_at(std::num::NonZeroUsize::new(1).unwrap(), turso::Value::Text(table.to_string().into()));

        loop {
            match stmt.step()? {
                turso::StepResult::Done => return Ok(false),
                turso::StepResult::Row => {
                    return Ok(true); // Found a row, table exists
                }
                turso::StepResult::IO => {
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
    fn conn(&self) -> Result<&std::sync::Arc<turso::Connection>> {
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
        self.get()?
            .query(table, columns, expr, limit, offset, sort)
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

    fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?.insert_only(table, columns, values)
    }

    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.get()?
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
fn sqlval_to_turso(val: &SqlValRef<'_>) -> turso::Value {
    use SqlValRef::*;
    match val {
        Bool(b) => turso::Value::Integer(*b as i64),
        Int(i) => turso::Value::Integer(*i as i64),
        BigInt(i) => turso::Value::Integer(*i),
        Real(r) => turso::Value::Float(*r),
        Text(t) => turso::Value::Text(t.to_string().into()),
        Blob(b) => turso::Value::Blob(b.to_vec()),
        #[cfg(feature = "json")]
        Json(v) => turso::Value::Text(serde_json::to_string(v).unwrap().into()),
        #[cfg(feature = "datetime")]
        Date(date) => {
            let f = date.format(TURSO_DATE_FORMAT);
            turso::Value::Text(f.to_string().into())
        }
        #[cfg(feature = "datetime")]
        Timestamp(dt) => {
            let f = dt.format(TURSO_DT_FORMAT);
            turso::Value::Text(f.to_string().into())
        }
        Null => turso::Value::Null,
        #[cfg(feature = "pg")]
        Custom(_) => panic!("Custom types not supported in turso"),
    }
}

fn turso_value_to_sqlval(val: &turso::Value) -> Result<SqlVal> {
    match val {
        turso::Value::Null => Ok(SqlVal::Null),
        turso::Value::Integer(i) => Ok(SqlVal::BigInt(*i)),
        turso::Value::Float(r) => Ok(SqlVal::Real(*r)),
        turso::Value::Text(t) => Ok(SqlVal::Text(t.to_string())),
        turso::Value::Blob(b) => Ok(SqlVal::Blob(b.clone())),
    }
}

fn turso_value_to_sqlval_typed(val: &turso::Value, ty: &SqlType) -> Result<SqlVal> {
    if matches!(val, turso::Value::Null) {
        return Ok(SqlVal::Null);
    }

    Ok(match ty {
        SqlType::Bool => {
            let i = match val {
                turso::Value::Integer(i) => *i,
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
                turso::Value::Integer(i) => *i,
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
                turso::Value::Integer(i) => *i,
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
                turso::Value::Float(r) => *r,
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
                turso::Value::Text(t) => t.to_string(),
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
                turso::Value::Text(t) => t,
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
                turso::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Date, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Date(NaiveDate::parse_from_str(t.as_str(), TURSO_DATE_FORMAT)?)
        }
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => {
            let t = match val {
                turso::Value::Text(t) => t,
                _ => {
                    return Err(Error::Internal(format!(
                        "Expected text for Timestamp, got {:?}",
                        val
                    )))
                }
            };
            SqlVal::Timestamp(NaiveDateTime::parse_from_str(t.as_str(), TURSO_DT_FORMAT)?)
        }
        SqlType::Blob => {
            let b = match val {
                turso::Value::Blob(b) => b.to_vec(),
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

/// Convert a Turso value to SqlVal without type information.
/// This infers the SqlVal type from the Turso value type.
#[allow(dead_code)]
fn turso_value_to_sqlval_untyped(val: &turso::Value) -> Result<SqlVal> {
    Ok(match val {
        turso::Value::Null => SqlVal::Null,
        turso::Value::Integer(i) => SqlVal::BigInt(*i),
        turso::Value::Float(r) => SqlVal::Real(*r),
        turso::Value::Text(t) => SqlVal::Text(t.to_string()),
        turso::Value::Blob(b) => SqlVal::Blob(b.clone()),
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
    // NOTE: Turso has a known limitation with ALTER TABLE RENAME operations within transactions
    // The error "table being renamed should be in schema" occurs because libSQL's schema
    // tracking doesn't properly register tables created in the same transaction.
    // For migrations that require column changes, consider skipping unmigrate for Turso.
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
    placeholder_source: &mut TursoPlaceholderSource,
    out: &mut impl Write,
) {
    // Subqueries should already be transformed by transform_subqueries before reaching here
    // So we can just use the default helper implementation
    helper::sql_for_expr(expr, sql_for_expr, values, placeholder_source, out)
}

#[derive(Debug)]
struct TursoPlaceholderSource;
impl TursoPlaceholderSource {
    fn new() -> Self {
        TursoPlaceholderSource {}
    }
}
impl helper::PlaceholderSource for TursoPlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<'_, str> {
        // Turso placeholder is always a question mark (SQLite-compatible)
        Cow::Borrowed("?")
    }
}
