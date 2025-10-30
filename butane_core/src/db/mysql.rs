//! MySQL database backend
use std::borrow::Cow;
use std::fmt::{Debug, Write};

use async_trait::async_trait;
#[cfg(feature = "datetime")]
use chrono::{Datelike, NaiveDate, Timelike};
use mysql_async as mysql;
use mysql_async::prelude::*;
use sqlparser;
use tokio::sync::Mutex;

use super::connmethods::VecRows;
use super::helper::{self, PlaceholderSource};
use crate::db::{
    Backend, BackendConnectionAsync as BackendConnection, BackendRow,
    BackendTransactionAsync as BackendTransaction, Column, Connection, ConnectionAsync,
    ConnectionMethodsAsync as ConnectionMethods, RawQueryResult, SyncAdapter,
    TransactionAsync as Transaction,
};
use crate::migrations::adb::{AColumn, ARef, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Expr};
use crate::{debug, query, Error, Result, SqlType, SqlVal, SqlValRef};

// MySQL-specific identifier quoting using backticks
fn quote_mysql_identifier(word: &str) -> Cow<'_, str> {
    if sqlparser::keywords::ALL_KEYWORDS.contains(&word.to_uppercase().as_str()) {
        format!("`{}`", word).into()
    } else {
        word.into()
    }
}

/// The name of the MySQL backend.
pub const BACKEND_NAME: &str = "mysql";

/// MySQL [`Backend`] implementation.
#[derive(Debug, Default, Clone)]
pub struct MySqlBackend;
impl MySqlBackend {
    pub fn new() -> MySqlBackend {
        MySqlBackend {}
    }
}

#[async_trait]
impl Backend for MySqlBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn row_id_column(&self) -> Option<&'static str> {
        None
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        let mut current: ADB = (*current).clone();
        let mut lines = ops
            .iter()
            .map(|o| sql_for_op(&mut current, o))
            .collect::<Result<Vec<String>>>()?;
        lines.retain(|s| !s.is_empty());
        Ok(lines.join("\n"))
    }

    fn connect(&self, path: &str) -> Result<Connection> {
        debug!("MySQL connecting via sync adapter");
        let conn = SyncAdapter::new(self.clone())?.connect(path)?;
        Ok(conn)
    }

    async fn connect_async(&self, path: &str) -> Result<ConnectionAsync> {
        Ok(ConnectionAsync {
            conn: Box::new(MySqlConnection::open(path).await?),
        })
    }
}

/// MySQL database connection.
pub struct MySqlConnection {
    #[cfg(feature = "debug")]
    params: Box<str>,
    conn: Mutex<mysql::Conn>,
}

impl MySqlConnection {
    async fn open(params: &str) -> Result<Self> {
        let opts = mysql::OptsBuilder::from_opts(params);
        let conn = mysql::Conn::new(opts).await?;
        Ok(Self {
            #[cfg(feature = "debug")]
            params: params.into(),
            conn: Mutex::new(conn),
        })
    }
}
impl MySqlConnectionLike for MySqlConnection {
    fn conn_ref(&self) -> &Mutex<mysql::Conn> {
        &self.conn
    }
}

#[async_trait]
impl BackendConnection for MySqlConnection {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let trans = Box::new(MySqlTransaction::new(&self.conn).await?);
        Ok(Transaction::new(trans))
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(MySqlBackend {})
    }
    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
    fn is_closed(&self) -> bool {
        false
    }
}
impl Debug for MySqlConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("MySqlConnection");
        #[cfg(feature = "debug")]
        d.field("params", &self.params);
        d.finish()
    }
}

/// Shared functionality between connection and
/// transaction. Implementation detail. Semver exempt.
trait MySqlConnectionLike {
    fn conn_ref(&self) -> &Mutex<mysql::Conn>;

    async fn execute_impl(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {sql}");
        }
        self.conn_ref().lock().await.query_drop(sql).await?;
        Ok(())
    }

    async fn query_impl<'a>(
        &'a self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[query::Order]>,
    ) -> Result<RawQueryResult<'a>> {
        let mut sqlquery = String::new();
        mysql_sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();

        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            mysql_sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut MySqlPlaceholderSource::new(),
                &mut sqlquery,
            );
        }

        if let Some(order) = order {
            mysql_sql_order(order, &mut sqlquery)
        }

        if let Some(limit) = limit {
            if let Some(offset) = offset {
                mysql_sql_offset(offset, Some(limit), &mut sqlquery);
            } else {
                helper::sql_limit(limit, &mut sqlquery);
            }
        } else if let Some(offset) = offset {
            mysql_sql_offset(offset, None, &mut sqlquery);
        }

        if cfg!(feature = "log") {
            debug!("query sql {sqlquery}");
        }

        let params: Vec<mysql::Value> = values.iter().map(sqlval_to_mysql_value).collect();
        let rows: Vec<mysql::Row> = self.conn_ref().lock().await.exec(&sqlquery, params).await?;

        Ok(Box::new(VecRows::new(rows)))
    }

    async fn insert_returning_pk_impl(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        let mut sql = String::new();
        mysql_sql_insert_with_placeholders(
            table,
            columns,
            &mut MySqlPlaceholderSource::new(),
            &mut sql,
        );

        if cfg!(feature = "log") {
            debug!("insert sql {sql}");
        }

        let params: Vec<mysql::Value> = values.iter().map(sqlvalref_to_mysql_value).collect();
        self.conn_ref().lock().await.exec_drop(&sql, params).await?;

        let last_id: Vec<mysql::Row> = self.conn_ref()
            .lock().await
            .query("SELECT LAST_INSERT_ID()")
            .await?;

        if let Some(row) = last_id.first() {
            mysql_value_to_sqlval(&row.get(0).unwrap(), pkcol.ty())
        } else {
            Err(Error::Internal(
                "could not get last insert id".to_string(),
            ))
        }
    }

    async fn insert_only_impl(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        mysql_sql_insert_with_placeholders(
            table,
            columns,
            &mut MySqlPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<mysql::Value> = values.iter().map(sqlvalref_to_mysql_value).collect();
        self.conn_ref().lock().await.exec_drop(&sql, params).await?;
        Ok(())
    }

    async fn insert_or_replace_impl(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        sql_insert_or_replace_with_placeholders(table, columns, pkcol, &mut sql);
        let params: Vec<mysql::Value> = values.iter().map(sqlvalref_to_mysql_value).collect();
        self.conn_ref().lock().await.exec_drop(&sql, params).await?;
        Ok(())
    }

    async fn update_impl(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        let mut sql = String::new();
        mysql_sql_update_with_placeholders(
            table,
            pkcol,
            columns,
            &mut MySqlPlaceholderSource::new(),
            &mut sql,
        );

        if cfg!(feature = "log") {
            debug!("update sql {sql}");
        }

        let mut params: Vec<mysql::Value> = values.iter().map(sqlvalref_to_mysql_value).collect();
        params.push(sqlvalref_to_mysql_value(&pk));

        self.conn_ref().lock().await.exec_drop(&sql, params).await?;
        Ok(())
    }

    async fn delete_impl(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.delete_where_impl(table, BoolExpr::Eq(pkcol, Expr::Val(pk)))
            .await?;
        Ok(())
    }

    async fn delete_where_impl(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(
            &mut sql,
            "DELETE FROM {} WHERE ",
            quote_mysql_identifier(table)
        )
        .unwrap();
        mysql_sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut MySqlPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<mysql::Value> = values.iter().map(sqlval_to_mysql_value).collect();
        let mut guard = self.conn_ref().lock().await;
        let result = guard.exec_iter(&sql, params).await?;
        Ok(result.affected_rows() as usize)
    }

    async fn has_table_impl(&self, table: &str) -> Result<bool> {
        let rows: Vec<mysql::Row> = self.conn_ref()
            .lock().await
            .exec(
                "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
                (table,),
            )
            .await?;

        Ok(!rows.is_empty())
    }
}

#[async_trait]
impl ConnectionMethods for MySqlConnection {
    async fn execute(&self, sql: &str) -> Result<()> {
        self.execute_impl(sql).await
    }

    async fn query<'a>(
        &'a self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[query::Order]>,
    ) -> Result<RawQueryResult<'a>> {
        self.query_impl(table, columns, expr, limit, offset, order).await
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.insert_returning_pk_impl(table, columns, pkcol, values).await
    }
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.insert_only_impl(table, columns, values).await
    }
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.insert_or_replace_impl(table, columns, pkcol, values).await
    }
    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.update_impl(table, pkcol, pk, columns, values).await
    }
    async fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.delete_impl(table, pkcol, pk).await
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.delete_where_impl(table, expr).await
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        self.has_table_impl(table).await
    }
}

impl MySqlConnectionLike for MySqlTransaction<'_> {
    fn conn_ref(&self) -> &Mutex<mysql::Conn> {
        &self.conn
    }
}

#[async_trait]
impl ConnectionMethods for MySqlTransaction<'_> {
    async fn execute(&self, sql: &str) -> Result<()> {
        // Execute within transaction context
        self.conn_ref().lock().await.query_drop(sql).await?;
        Ok(())
    }

    async fn query<'a>(
        &'a self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[query::Order]>,
    ) -> Result<RawQueryResult<'a>> {
        self.query_impl(table, columns, expr, limit, offset, order).await
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.insert_returning_pk_impl(table, columns, pkcol, values).await
    }
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.insert_only_impl(table, columns, values).await
    }
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.insert_or_replace_impl(table, columns, pkcol, values).await
    }
    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.update_impl(table, pkcol, pk, columns, values).await
    }
    async fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.delete_impl(table, pkcol, pk).await
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.delete_where_impl(table, expr).await
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        self.has_table_impl(table).await
    }
}

struct MySqlTransaction<'c> {
    conn: &'c Mutex<mysql::Conn>,
    committed_or_rolled_back: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl<'c> MySqlTransaction<'c> {
    async fn new(conn: &'c Mutex<mysql::Conn>) -> Result<Self> {
        conn.lock().await.query_drop("BEGIN").await?;
        Ok(MySqlTransaction {
            conn,
            committed_or_rolled_back: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    fn already_consumed() -> Error {
        Error::Internal("transaction has already been consumed".to_string())
    }
}

impl Debug for MySqlTransaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MySqlTransaction")
            .field("conn", &"<connection>")
            .finish()
    }
}

#[async_trait]
impl<'c> BackendTransaction<'c> for MySqlTransaction<'c> {
    async fn commit(&mut self) -> Result<()> {
        if self.committed_or_rolled_back.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(Self::already_consumed());
        }
        self.conn.lock().await.query_drop("COMMIT").await?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        if self.committed_or_rolled_back.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(Self::already_consumed());
        }
        self.conn.lock().await.query_drop("ROLLBACK").await?;
        Ok(())
    }
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

impl<'c> Drop for MySqlTransaction<'c> {
    fn drop(&mut self) {
        // If the transaction hasn't been explicitly committed or rolled back, warn and mark as rolled back
        if !self.committed_or_rolled_back.swap(true, std::sync::atomic::Ordering::SeqCst) {
            log::warn!("Transaction dropped without explicit commit or rollback - this may lead to uncommitted changes");
            // Unfortunately, we can't perform async operations in Drop
            // The user should explicitly call rollback() if they want to ensure rollback
        }
    }
}

fn sqlval_to_mysql_value(v: &SqlVal) -> mysql::Value {
    sqlvalref_to_mysql_value(&v.as_ref())
}

fn sqlvalref_to_mysql_value(v: &SqlValRef<'_>) -> mysql::Value {
    use SqlValRef::*;
    match v {
        Bool(b) => mysql::Value::Int(*b as i64),
        Int(i) => mysql::Value::Int(*i as i64),
        BigInt(i) => mysql::Value::Int(*i),
        Real(r) => mysql::Value::Double(*r),
        Text(t) => mysql::Value::Bytes(t.as_bytes().to_vec()),
        Blob(b) => mysql::Value::Bytes((*b).to_vec()),
        #[cfg(feature = "json")]
        Json(v) => mysql::Value::Bytes(v.to_string().as_bytes().to_vec()),
        #[cfg(feature = "datetime")]
        Date(d) => mysql::Value::Date(
            d.year() as u16,
            d.month() as u8,
            d.day() as u8,
            0,
            0,
            0,
            0,
        ),
        #[cfg(feature = "datetime")]
        Timestamp(dt) => mysql::Value::Date(
            dt.year() as u16,
            dt.month() as u8,
            dt.day() as u8,
            dt.hour() as u8,
            dt.minute() as u8,
            dt.second() as u8,
            dt.and_utc().timestamp_subsec_micros(),
        ),
        Null => mysql::Value::NULL,
        Custom(_) => panic!("Custom types not yet supported for MySQL"),
    }
}

fn mysql_value_to_sqlval(v: &mysql::Value, ty: &SqlType) -> Result<SqlVal> {
    use mysql::Value::*;
    match (v, ty) {
        (Int(i), SqlType::Bool) => Ok(SqlVal::Bool(*i != 0)),
        (Int(i), SqlType::Int) => Ok(SqlVal::Int(*i as i32)),
        (Int(i), SqlType::BigInt) => Ok(SqlVal::BigInt(*i)),
        (Bytes(b), SqlType::BigInt) => {
            // MySQL sometimes returns integers as bytes (especially for auto-increment)
            let s = String::from_utf8(b.clone()).map_err(|e| Error::Internal(e.to_string()))?;
            let i = s.parse::<i64>().map_err(|e| Error::Internal(e.to_string()))?;
            Ok(SqlVal::BigInt(i))
        },
        (Bytes(b), SqlType::Int) => {
            // MySQL sometimes returns integers as bytes
            let s = String::from_utf8(b.clone()).map_err(|e| Error::Internal(e.to_string()))?;
            let i = s.parse::<i32>().map_err(|e| Error::Internal(e.to_string()))?;
            Ok(SqlVal::Int(i))
        },
        (Float(f), SqlType::Real) => Ok(SqlVal::Real(*f as f64)),
        (Double(d), SqlType::Real) => Ok(SqlVal::Real(*d)),
        (Bytes(b), SqlType::Text) => Ok(SqlVal::Text(
            String::from_utf8(b.clone()).map_err(|e| Error::Internal(e.to_string()))?,
        )),
        (Bytes(b), SqlType::Blob) => Ok(SqlVal::Blob(b.clone())),
        #[cfg(feature = "json")]
        (Bytes(b), SqlType::Json) => {
            let s = String::from_utf8(b.clone()).map_err(|e| Error::Internal(e.to_string()))?;
            let json: serde_json::Value =
                serde_json::from_str(&s).map_err(|e| Error::Internal(e.to_string()))?;
            Ok(SqlVal::Json(json))
        }
        #[cfg(feature = "datetime")]
        (Date(y, m, d, h, mi, s, us), SqlType::Timestamp) => {
            let dt = NaiveDate::from_ymd_opt(*y as i32, *m as u32, *d as u32)
                .ok_or_else(|| Error::Internal("invalid date".to_string()))?
                .and_hms_micro_opt(*h as u32, *mi as u32, *s as u32, *us)
                .ok_or_else(|| Error::Internal("invalid time".to_string()))?;
            Ok(SqlVal::Timestamp(dt))
        }
        #[cfg(feature = "datetime")]
        (Date(y, m, d, _, _, _, _), SqlType::Date) => {
            let date = NaiveDate::from_ymd_opt(*y as i32, *m as u32, *d as u32)
                .ok_or_else(|| Error::Internal("invalid date".to_string()))?;
            Ok(SqlVal::Date(date))
        }
        (NULL, _) => Ok(SqlVal::Null),
        _ => Err(Error::SqlResultTypeMismatch {
            col: "unknown".to_string(),
            detail: format!("cannot convert {:?} to {:?}", v, ty),
        }),
    }
}

impl BackendRow for mysql::Row {
    fn get(&self, idx: usize, ty: SqlType) -> Result<SqlValRef<'_>> {
        let value: mysql::Value = self.get(idx).ok_or_else(|| {
            Error::Internal(format!("column index {} out of bounds", idx))
        })?;

        let sqlval = mysql_value_to_sqlval(&value, &ty)?;

        match sqlval {
            SqlVal::Bool(b) => Ok(SqlValRef::Bool(b)),
            SqlVal::Int(i) => Ok(SqlValRef::Int(i)),
            SqlVal::BigInt(i) => Ok(SqlValRef::BigInt(i)),
            SqlVal::Real(r) => Ok(SqlValRef::Real(r)),
            SqlVal::Text(s) => {
                Ok(SqlValRef::Text(Box::leak(s.into_boxed_str())))
            }
            SqlVal::Blob(b) => {
                Ok(SqlValRef::Blob(Box::leak(b.into_boxed_slice())))
            }
            #[cfg(feature = "json")]
            SqlVal::Json(v) => {
                Ok(SqlValRef::Json(v))
            }
            #[cfg(feature = "datetime")]
            SqlVal::Date(d) => Ok(SqlValRef::Date(d)),
            #[cfg(feature = "datetime")]
            SqlVal::Timestamp(dt) => Ok(SqlValRef::Timestamp(dt)),
            SqlVal::Null => Ok(SqlValRef::Null),
            SqlVal::Custom(_) => Err(Error::Internal("Custom types not supported".to_string())),
        }
    }

    fn len(&self) -> usize {
        self.len()
    }
}

fn mysql_sql_for_expr<W>(
    expr: query::Expr,
    values: &mut Vec<SqlVal>,
    pls: &mut MySqlPlaceholderSource,
    w: &mut W,
) where
    W: Write,
{
    use query::Expr;

    match expr {
        Expr::Column(name) => w.write_str(&quote_mysql_identifier(name)).unwrap(),
        Expr::Val(v) => match v {
            // No risk of SQL injection with integers and the
            // different sizes are tricky with the PG backend's binary
            // protocol
            SqlVal::Int(i) => write!(w, "{i}").unwrap(),
            SqlVal::BigInt(i) => write!(w, "{i}").unwrap(),
            _ => {
                values.push(v);
                w.write_str(&pls.next_placeholder()).unwrap()
            }
        },
        Expr::Placeholder => {
            w.write_str(&pls.next_placeholder()).unwrap();
        },
        Expr::Condition(cond) => {
            mysql_sql_for_bool_expr(*cond, values, pls, w);
        },
    }
}

fn mysql_sql_for_bool_expr<W>(
    expr: BoolExpr,
    values: &mut Vec<SqlVal>,
    pls: &mut MySqlPlaceholderSource,
    w: &mut W,
) where
    W: Write,
{
    use query::BoolExpr::*;
    match expr {
        AllOf(conditions) => {
            write!(w, "(").unwrap();
            for (i, condition) in conditions.into_iter().enumerate() {
                if i > 0 {
                    write!(w, " AND ").unwrap();
                }
                mysql_sql_for_bool_expr(condition, values, pls, w);
            }
            write!(w, ")").unwrap();
        }
        And(left, right) => {
            write!(w, "(").unwrap();
            mysql_sql_for_bool_expr(*left, values, pls, w);
            write!(w, " AND ").unwrap();
            mysql_sql_for_bool_expr(*right, values, pls, w);
            write!(w, ")").unwrap();
        }
        Or(left, right) => {
            write!(w, "(").unwrap();
            mysql_sql_for_bool_expr(*left, values, pls, w);
            write!(w, " OR ").unwrap();
            mysql_sql_for_bool_expr(*right, values, pls, w);
            write!(w, ")").unwrap();
        }
        Not(condition) => {
            write!(w, "NOT (").unwrap();
            mysql_sql_for_bool_expr(*condition, values, pls, w);
            write!(w, ")").unwrap();
        }
        Eq(col, expr) => {
            // Handle NULL comparisons with IS NULL instead of = NULL
            if let query::Expr::Val(SqlVal::Null) = expr {
                write!(w, "{} IS NULL", quote_mysql_identifier(col)).unwrap();
            } else {
                write!(w, "{} = ", quote_mysql_identifier(col)).unwrap();
                mysql_sql_for_expr(expr, values, pls, w);
            }
        }
        Ne(col, expr) => {
            // Handle NULL comparisons with IS NOT NULL instead of != NULL
            if let query::Expr::Val(SqlVal::Null) = expr {
                write!(w, "{} IS NOT NULL", quote_mysql_identifier(col)).unwrap();
            } else {
                write!(w, "{} != ", quote_mysql_identifier(col)).unwrap();
                mysql_sql_for_expr(expr, values, pls, w);
            }
        }
        Lt(col, expr) => {
            write!(w, "{} < ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_for_expr(expr, values, pls, w);
        }
        Le(col, expr) => {
            write!(w, "{} <= ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_for_expr(expr, values, pls, w);
        }
        Gt(col, expr) => {
            write!(w, "{} > ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_for_expr(expr, values, pls, w);
        }
        Ge(col, expr) => {
            write!(w, "{} >= ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_for_expr(expr, values, pls, w);
        }
        Like(col, pattern) => {
            write!(w, "{} LIKE ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_for_expr(pattern, values, pls, w);
        }
        Subquery {
            col,
            tbl2,
            tbl2_col,
            expr,
        } => {
            write!(w, "{} IN (SELECT {} FROM {} WHERE ",
                quote_mysql_identifier(col),
                quote_mysql_identifier(tbl2_col),
                quote_mysql_identifier(&tbl2)
            ).unwrap();
            mysql_sql_for_bool_expr(*expr, values, pls, w);
            write!(w, ")").unwrap();
        }
        SubqueryJoin {
            col,
            tbl2,
            col2,
            joins,
            expr,
        } => {
            write!(w, "{} IN (SELECT ", quote_mysql_identifier(col)).unwrap();
            mysql_sql_column(col2.clone(), w);
            write!(w, " FROM {} ", quote_mysql_identifier(&tbl2)).unwrap();
            mysql_sql_joins(&joins, w);
            write!(w, " WHERE ").unwrap();
            mysql_sql_for_bool_expr(*expr, values, pls, w);
            write!(w, ")").unwrap();
        }
        In(col, vals) => {
            write!(w, "{} IN (", quote_mysql_identifier(col)).unwrap();
            for (i, val) in vals.into_iter().enumerate() {
                if i > 0 {
                    write!(w, ", ").unwrap();
                }
                values.push(val);
                write!(w, "{}", pls.next_placeholder()).unwrap();
            }
            write!(w, ")").unwrap();
        }
        True => {
            write!(w, "TRUE").unwrap();
        }
    }
}

// MySQL-specific SQL generation functions that use backtick quoting
fn mysql_sql_select(columns: &[Column], table: &str, w: &mut impl Write) {
    write!(w, "SELECT ").unwrap();
    mysql_list_columns(columns, w);
    write!(w, " FROM {}", quote_mysql_identifier(table)).unwrap();
}

fn mysql_sql_insert_with_placeholders(
    table: &str,
    columns: &[Column],
    pls: &mut impl helper::PlaceholderSource,
    w: &mut impl Write,
) {
    write!(w, "INSERT INTO {} ", quote_mysql_identifier(table)).unwrap();
    if !columns.is_empty() {
        write!(w, "(").unwrap();
        mysql_list_columns(columns, w);
        write!(w, ") VALUES (").unwrap();
        columns.iter().fold("", |sep, _| {
            write!(w, "{}{}", sep, pls.next_placeholder()).unwrap();
            ", "
        });
        write!(w, ")").unwrap();
    } else {
        // MySQL doesn't support DEFAULT VALUES, use () VALUES () instead
        write!(w, "() VALUES ()").unwrap();
    }
}

fn mysql_sql_update_with_placeholders(
    table: &str,
    pkcol: Column,
    columns: &[Column],
    pls: &mut impl helper::PlaceholderSource,
    w: &mut impl Write,
) {
    write!(w, "UPDATE {} SET ", quote_mysql_identifier(table)).unwrap();
    columns.iter().fold("", |sep, c| {
        write!(
            w,
            "{}{} = {}",
            sep,
            quote_mysql_identifier(c.name()),
            pls.next_placeholder()
        )
        .unwrap();
        ", "
    });
    write!(
        w,
        " WHERE {} = {}",
        quote_mysql_identifier(pkcol.name()),
        pls.next_placeholder()
    )
    .unwrap();
}

fn mysql_list_columns(columns: &[Column], w: &mut impl Write) {
    let mut colnames: Vec<&'static str> = Vec::new();
    columns.iter().for_each(|c| colnames.push(c.name()));
    write!(
        w,
        "{}",
        colnames
            .iter()
            .map(|x| quote_mysql_identifier(x))
            .collect::<Vec<Cow<str>>>()
            .join(", ")
    )
    .unwrap();
}

// MySQL-specific JOIN functions that use backtick quoting
fn mysql_sql_joins(joins: &[query::Join], w: &mut impl Write) {
    for join in joins {
        match join {
            query::Join::Inner {
                join_table,
                col1,
                col2,
            } => {
                // INNER JOIN <join_table> ON <col1> = <col2>
                write!(w, " INNER JOIN {} ON ", quote_mysql_identifier(join_table)).unwrap();
                mysql_sql_column(col1.clone(), w);
                w.write_str(" = ").unwrap();
                mysql_sql_column(col2.clone(), w);
            }
        }
    }
}

fn mysql_sql_column(col: query::Column, w: &mut impl Write) {
    match col.table() {
        Some(table) => write!(
            w,
            "{}.{}",
            quote_mysql_identifier(table),
            quote_mysql_identifier(col.name())
        ),
        None => w.write_str(&quote_mysql_identifier(col.name())),
    }
    .unwrap()
}

// MySQL-specific OFFSET handling - MySQL requires LIMIT when using OFFSET
fn mysql_sql_offset(offset: i32, limit: Option<i32>, w: &mut impl Write) {
    if let Some(limit) = limit {
        write!(w, " LIMIT {limit} OFFSET {offset}").unwrap();
    } else {
        // MySQL requires LIMIT when using OFFSET, use a very large number
        write!(w, " LIMIT 18446744073709551615 OFFSET {offset}").unwrap();
    }
}

// MySQL-specific ORDER BY handling using backticks for identifier quoting
fn mysql_sql_order(order: &[query::Order], w: &mut impl Write) {
    write!(w, " ORDER BY ").unwrap();
    order.iter().fold("", |sep, o| {
        let sql_dir = match o.direction {
            query::OrderDirection::Ascending => "ASC",
            query::OrderDirection::Descending => "DESC",
        };
        write!(w, "{}{} {}", sep, quote_mysql_identifier(o.column), sql_dir).unwrap();
        ", "
    });
}

fn sql_for_op(current: &mut ADB, op: &Operation) -> Result<String> {
    match op {
        Operation::AddTable(table) => Ok(create_table(table, false)?),
        Operation::AddTableConstraints(table) => Ok(create_table_fkey_constraints(table)),
        Operation::AddTableIfNotExists(table) => Ok(create_table(table, true)?),
        Operation::RemoveTable(name) => Ok(drop_table(name)),
        Operation::RemoveTableConstraints(table) => remove_table_fkey_constraints(table),
        Operation::AddColumn(tbl, col) => add_column(tbl, col),
        Operation::RemoveColumn(tbl, name) => Ok(remove_column(tbl, name)),
        Operation::ChangeColumn(tbl, old, new) => {
            let table = current.get_table(tbl);
            if let Some(table) = table {
                change_column(table, old, new)
            } else {
                crate::warn!(
                    "Cannot alter column {} from table {} that does not exist",
                    &old.name(),
                    tbl
                );
                Ok(String::new())
            }
        }
    }
}

fn create_table(table: &ATable, allow_exists: bool) -> Result<String> {
    let coldefs = table
        .columns
        .iter()
        .map(define_column)
        .collect::<Result<Vec<String>>>()?
        .join(",\n");
    let modifier = if allow_exists { "IF NOT EXISTS " } else { "" };
    Ok(format!(
        "CREATE TABLE {}{} (\n{}\n);",
        modifier,
        quote_mysql_identifier(&table.name),
        coldefs
    ))
}

fn create_table_fkey_constraints(table: &ATable) -> String {
    table
        .columns
        .iter()
        .filter(|column| column.reference().is_some())
        .map(|column| define_fkey_constraint(&table.name, column))
        .collect::<Vec<String>>()
        .join("\n")
}

fn remove_table_fkey_constraints(table: &ATable) -> Result<String> {
    Ok(table
        .columns
        .iter()
        .filter(|column| column.reference().is_some())
        .map(|column| drop_fkey_constraints(table, column))
        .collect::<Result<Vec<String>>>()?
        .join("\n"))
}

fn define_column(col: &AColumn) -> Result<String> {
    let mut constraints: Vec<String> = Vec::new();
    if !col.nullable() {
        constraints.push("NOT NULL".to_string());
    }
    if col.is_pk() {
        constraints.push("PRIMARY KEY".to_string());
    }
    if col.unique() {
        constraints.push("UNIQUE".to_string());
    }
    if constraints.is_empty() {
        return Ok(format!(
            "{} {}",
            quote_mysql_identifier(col.name()),
            col_sqltype(col)?,
        ));
    }
    Ok(format!(
        "{} {} {}",
        quote_mysql_identifier(col.name()),
        col_sqltype(col)?,
        constraints.join(" ")
    ))
}

fn define_fkey_constraint(table_name: &str, column: &AColumn) -> String {
    let reference = column
        .reference()
        .as_ref()
        .expect("must have a references value");
    match reference {
        ARef::Literal(literal) => {
            let constraint_name = format!("{}_{}_fkey", table_name, column.name());
            format!(
                "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({});",
                quote_mysql_identifier(table_name),
                quote_mysql_identifier(&constraint_name),
                quote_mysql_identifier(column.name()),
                quote_mysql_identifier(literal.table_name()),
                quote_mysql_identifier(literal.column_name()),
            )
        }
        _ => panic!(),
    }
}

fn drop_fkey_constraints(table: &ATable, column: &AColumn) -> Result<String> {
    let mut modified_column = column.clone();
    modified_column.remove_reference();
    change_column(table, column, &modified_column)
}

fn col_sqltype(col: &AColumn) -> Result<Cow<'_, str>> {
    match col.typeid()? {
        TypeIdentifier::Name(name) => Ok(Cow::Owned(name)),
        TypeIdentifier::Ty(ty) => {
            if col.is_auto() {
                match ty {
                    SqlType::Int => Ok(Cow::Borrowed("INT AUTO_INCREMENT")),
                    SqlType::BigInt => Ok(Cow::Borrowed("BIGINT AUTO_INCREMENT")),
                    _ => Err(Error::InvalidAuto(col.name().to_string())),
                }
            } else {
                Ok(match ty {
                    SqlType::Bool => Cow::Borrowed("BOOLEAN"),
                    SqlType::Int => Cow::Borrowed("INT"),
                    SqlType::BigInt => Cow::Borrowed("BIGINT"),
                    SqlType::Real => Cow::Borrowed("DOUBLE"),
                    SqlType::Text => {
                        // MySQL TEXT columns can't be used in unique constraints without key length
                        // Use VARCHAR(255) for unique/primary key columns, TEXT for others
                        // Also use VARCHAR(255) for foreign key columns to match referenced columns
                        if col.unique() || col.is_pk() || col.reference().is_some() {
                            Cow::Borrowed("VARCHAR(255)")
                        } else {
                            Cow::Borrowed("TEXT")
                        }
                    },
                    #[cfg(feature = "datetime")]
                    SqlType::Date => Cow::Borrowed("DATE"),
                    #[cfg(feature = "datetime")]
                    SqlType::Timestamp => Cow::Borrowed("DATETIME(6)"), // Use microsecond precision
                    SqlType::Blob => {
                        // MySQL BLOB columns can't be used in unique constraints without key length
                        // Use VARBINARY(255) for unique/primary key columns, BLOB for others
                        // Also use VARBINARY(255) for foreign key columns to match referenced columns
                        if col.unique() || col.is_pk() || col.reference().is_some() {
                            Cow::Borrowed("VARBINARY(255)")
                        } else {
                            Cow::Borrowed("BLOB")
                        }
                    },
                    #[cfg(feature = "json")]
                    SqlType::Json => Cow::Borrowed("JSON"),
                    SqlType::Custom(_) => {
                        return Err(Error::Internal(
                            "Custom types not yet supported for MySQL".to_string(),
                        ))
                    }
                })
            }
        }
    }
}

fn drop_table(name: &str) -> String {
    format!("DROP TABLE {};", quote_mysql_identifier(name))
}

fn add_column(tbl_name: &str, col: &AColumn) -> Result<String> {
    let default: SqlVal = helper::column_default(col)?;
    let mut stmts = vec![format!(
        "ALTER TABLE {} ADD COLUMN {} DEFAULT {};",
        quote_mysql_identifier(tbl_name),
        define_column(col)?,
        helper::sql_literal_value(&default)?
    )];
    if col.reference().is_some() {
        stmts.push(define_fkey_constraint(tbl_name, col));
    }
    let result = stmts.join("\n");
    Ok(result)
}

fn remove_column(tbl_name: &str, name: &str) -> String {
    format!(
        "ALTER TABLE {} DROP COLUMN {};",
        quote_mysql_identifier(tbl_name),
        quote_mysql_identifier(name)
    )
}

fn change_column(table: &ATable, old: &AColumn, new: &AColumn) -> Result<String> {
    let tbl_name = &table.name;

    let mut stmts: Vec<String> = Vec::new();

    // MySQL requires CHANGE COLUMN for rename, MODIFY COLUMN for type/constraint changes
    if old.name() != new.name() {
        stmts.push(format!(
            "ALTER TABLE {} CHANGE COLUMN {} {} {};",
            quote_mysql_identifier(tbl_name),
            quote_mysql_identifier(old.name()),
            quote_mysql_identifier(new.name()),
            col_sqltype(new)?,
        ));
    } else if old.typeid()? != new.typeid()? || old.nullable() != new.nullable() {
        stmts.push(format!(
            "ALTER TABLE {} MODIFY COLUMN {} {};",
            quote_mysql_identifier(tbl_name),
            quote_mysql_identifier(old.name()),
            col_sqltype(new)?,
        ));
    }

    if old.is_pk() != new.is_pk() {
        if new.is_pk() {
            stmts.push(format!(
                "ALTER TABLE {} DROP PRIMARY KEY;",
                quote_mysql_identifier(tbl_name)
            ));
            stmts.push(format!(
                "ALTER TABLE {} ADD PRIMARY KEY ({});",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(new.name())
            ));
        }
    }

    if old.unique() != new.unique() {
        if new.unique() {
            stmts.push(format!(
                "ALTER TABLE {} ADD UNIQUE ({});",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(new.name())
            ));
        } else {
            stmts.push(format!(
                "ALTER TABLE {} DROP INDEX {};",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(old.name())
            ));
        }
    }

    if old.default() != new.default() {
        stmts.push(match new.default() {
            None => format!(
                "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(old.name())
            ),
            Some(val) => format!(
                "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(old.name()),
                helper::sql_literal_value(val)?
            ),
        });
    }

    if old.reference() != new.reference() {
        if old.reference().is_some() {
            // MySQL requires knowing the constraint name to drop it
            let constraint_name = format!("{}_{}_fkey", tbl_name, old.name());
            stmts.push(format!(
                "ALTER TABLE {} DROP FOREIGN KEY {};",
                quote_mysql_identifier(tbl_name),
                quote_mysql_identifier(&constraint_name)
            ));
        }
        if new.reference().is_some() {
            stmts.push(define_fkey_constraint(tbl_name, new));
        }
    }

    let result = stmts.join("\n");
    Ok(result)
}

pub fn sql_insert_or_replace_with_placeholders(
    table: &str,
    columns: &[Column],
    pkcol: &Column,
    w: &mut impl Write,
) {
    write!(w, "INSERT ").unwrap();
    write!(w, "INTO {} (", quote_mysql_identifier(table)).unwrap();
    mysql_list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold(true, |first, _| {
        if !first {
            write!(w, ", ").unwrap();
        }
        write!(w, "?").unwrap();
        false
    });
    write!(w, ")").unwrap();
    write!(w, " ON DUPLICATE KEY UPDATE ").unwrap();
    if columns.len() > 1 {
        columns
            .iter()
            .filter(|c| c.name() != pkcol.name())
            .fold(true, |first, c| {
                if !first {
                    write!(w, ", ").unwrap();
                }
                write!(w, "{} = VALUES({})", quote_mysql_identifier(c.name()), quote_mysql_identifier(c.name())).unwrap();
                false
            });
    } else {
        write!(w, "{} = {}", quote_mysql_identifier(pkcol.name()), quote_mysql_identifier(pkcol.name())).unwrap();
    }
}

#[derive(Debug)]
struct MySqlPlaceholderSource;

impl MySqlPlaceholderSource {
    fn new() -> Self {
        MySqlPlaceholderSource
    }
}

impl helper::PlaceholderSource for MySqlPlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<'_, str> {
        Cow::Borrowed("?")
    }
}
