//! Postgresql database backend
use std::borrow::Cow;
use std::fmt::{Debug, Write};

use async_trait::async_trait;
use bytes::BufMut;
#[cfg(feature = "datetime")]
use chrono::{NaiveDate, NaiveDateTime};
use futures_util::stream::StreamExt;
use tokio_postgres as postgres;
use tokio_postgres::GenericClient;

use super::connmethods::VecRows;
use super::helper;
use crate::custom::{SqlTypeCustom, SqlValRefCustom};
use crate::db::{
    Backend, BackendConnectionAsync as BackendConnection, BackendRow,
    BackendTransactionAsync as BackendTransaction, Column, Connection, ConnectionAsync,
    ConnectionMethodsAsync as ConnectionMethods, RawQueryResult, SyncAdapter,
    TransactionAsync as Transaction,
};
use crate::migrations::adb::{AColumn, ARef, ATable, Operation, TypeIdentifier, ADB};
use crate::query::{BoolExpr, Expr};
use crate::{debug, query, warn, Error, Result, SqlType, SqlVal, SqlValRef};

/// Postgres backend name.
pub const BACKEND_NAME: &str = "pg";
/// Internal row ordering field name.
pub const ROW_ID_COLUMN_NAME: &str = "ctid";

/// Postgres [`Backend`] implementation.
#[derive(Debug, Default, Clone)]
pub struct PgBackend;
impl PgBackend {
    pub fn new() -> PgBackend {
        PgBackend {}
    }
}

#[async_trait]
impl Backend for PgBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn row_id_column(&self) -> Option<&'static str> {
        Some(ROW_ID_COLUMN_NAME)
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        let mut current: ADB = (*current).clone();
        let mut lines = ops
            .iter()
            .map(|o| sql_for_op(&mut current, o))
            .collect::<Result<Vec<String>>>()?;
        lines.retain(|s| !s.is_empty());
        Ok(format!("{}\n", lines.join("\n")))
    }

    fn connect(&self, path: &str) -> Result<Connection> {
        debug!("Postgres connecting via sync adapter");
        let conn = SyncAdapter::new(self.clone())?.connect(path)?;
        Ok(conn)
    }

    async fn connect_async(&self, path: &str) -> Result<ConnectionAsync> {
        Ok(ConnectionAsync {
            conn: Box::new(PgConnection::open(path).await?),
        })
    }
}

/// Pg database connection.
pub struct PgConnection {
    #[cfg(feature = "debug")]
    params: Box<str>,
    client: postgres::Client,
}

impl PgConnection {
    async fn open(params: &str) -> Result<Self> {
        let client = Self::connect(params).await?;
        Ok(Self {
            #[cfg(feature = "debug")]
            params: params.into(),
            client,
        })
    }
    async fn connect(params: &str) -> Result<postgres::Client> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "tls")] {
                let connector = native_tls::TlsConnector::new()?;
                let connector = postgres_native_tls::MakeTlsConnector::new(connector);
            } else {
                let connector = postgres::NoTls;
            }
        }
        let (client, conn) = postgres::connect(params, connector).await?;
        tokio::spawn(async move {
            #[allow(unused_variables)] // used only when logging is enabled
            if let Err(e) = conn.await {
                warn!("Postgres connection error {}", e);
            }
        });
        Ok(client)
    }
}
impl PgConnectionLike for PgConnection {
    type Client = postgres::Client;
    fn client(&self) -> Result<&Self::Client> {
        Ok(&self.client)
    }
}

#[async_trait]
impl BackendConnection for PgConnection {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let trans: postgres::Transaction<'_> = self.client.transaction().await?;
        let trans = Box::new(PgTransaction::new(trans));
        Ok(Transaction::new(trans))
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(PgBackend {})
    }
    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
    fn is_closed(&self) -> bool {
        self.client.is_closed()
    }
}
impl Debug for PgConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("PgConnection");
        #[cfg(feature = "debug")]
        d.field("params", &self.params);
        // postgres::Client doesnt expose any internal state
        d.field("conn", &!self.is_closed());
        d.finish()
    }
}

type DynToSqlPg<'a> = dyn postgres::types::ToSql + Sync + 'a;

fn sqlval_for_pg_query(v: &SqlVal) -> &dyn postgres::types::ToSql {
    v as &dyn postgres::types::ToSql
}

fn sqlvalref_for_pg_query<'a>(v: &'a SqlValRef<'a>) -> &'a dyn postgres::types::ToSql {
    v as &dyn postgres::types::ToSql
}

/// Shared functionality between connection and transaction.
///
/// Implementation detail. Semver exempt.
trait PgConnectionLike {
    type Client: postgres::GenericClient + Send;
    fn client(&self) -> Result<&Self::Client>;
}

#[async_trait]
impl<T> ConnectionMethods for T
where
    T: PgConnectionLike + std::marker::Sync,
{
    async fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {sql}");
        }
        // Note, let binding exists only so that the self.client() reference is not held across the await
        let future = self.client()?.batch_execute(sql.as_ref());
        future.await?;
        Ok(())
    }

    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[query::Order]>,
    ) -> Result<RawQueryResult<'c>> {
        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();
        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut PgPlaceholderSource::new(),
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

        if cfg!(feature = "log") {
            debug!("query sql {sqlquery}");
        }

        let types: Vec<postgres::types::Type> = values.iter().map(pgtype_for_val).collect();
        let future = self.client()?.prepare_typed(&sqlquery, types.as_ref());
        let stmt = future.await?;
        let mut rowvec = Vec::<postgres::Row>::new();
        let future = self
            .client()?
            .query_raw(&stmt, values.iter().map(sqlval_for_pg_query));
        let rowstream = future.await.map_err(Error::Postgres)?;
        let mut rowstream = Box::pin(rowstream);
        while let Some(r) = rowstream.next().await {
            let r = r?;
            check_columns(&r, columns)?;
            rowvec.push(r);
        }
        Ok(Box::new(VecRows::new(rowvec)))
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        write!(&mut sql, " RETURNING {}", pkcol.name()).unwrap();
        if cfg!(feature = "log") {
            debug!("insert sql {sql}");
        }

        // use query instead of execute so we can get our result back
        let future = self
            .client()?
            .query_raw(sql.as_str(), values.iter().map(sqlvalref_for_pg_query));
        let pk_stream = future
            .await
            .map_err(Error::Postgres)?
            .map(|r| r.map(|x| sql_val_from_postgres(&x, 0, pkcol)));
        Box::pin(pk_stream)
            .next()
            .await
            .ok_or(Error::Internal(("could not get pk").to_string()))??
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let future = self.client()?.execute(sql.as_str(), params.as_slice());
        future.await?;
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
        sql_insert_or_replace_with_placeholders(table, columns, pkcol, &mut sql);
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let future = self.client()?.execute(sql.as_str(), params.as_slice());
        future.await?;
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let placeholder_values = [values, &[pk]].concat();
        let params: Vec<&DynToSqlPg> = placeholder_values
            .iter()
            .map(|v| v as &DynToSqlPg)
            .collect();
        if cfg!(feature = "log") {
            debug!("update sql {sql}");
        }
        let future = self.client()?.execute(sql.as_str(), params.as_slice());
        future.await?;
        Ok(())
    }
    async fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
        self.delete_where(table, BoolExpr::Eq(pkcol, Expr::Val(pk)))
            .await?;
        Ok(())
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let future = self.client()?.execute(sql.as_str(), params.as_slice());
        let cnt = future.await?;
        Ok(cnt as usize)
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        // future improvement, should be schema-aware
        let future = self
            .client()?
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_name=$1;");
        let stmt = future.await?;
        let tableref: &[&(dyn postgres::types::ToSql + Sync)] = &[&table];
        let future = self.client()?.query(&stmt, tableref);
        let rows = future.await?;
        Ok(!rows.is_empty())
    }
}

struct PgTransaction<'c> {
    trans: Option<postgres::Transaction<'c>>,
}
impl<'c> PgTransaction<'c> {
    fn new(trans: postgres::Transaction<'c>) -> Self {
        PgTransaction { trans: Some(trans) }
    }
    fn get(&self) -> Result<&postgres::Transaction<'c>> {
        match &self.trans {
            Some(x) => Ok(x),
            None => Err(Self::already_consumed()),
        }
    }
    fn already_consumed() -> Error {
        Error::Internal("transaction has already been consumed".to_string())
    }
}
impl Debug for PgTransaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgTransaction")
            // postgres::Transaction doesnt expose any internal state
            .field("trans", &self.trans.is_some())
            .finish()
    }
}

impl<'c> PgConnectionLike for PgTransaction<'c> {
    type Client = postgres::Transaction<'c>;
    fn client(&self) -> Result<&Self::Client> {
        self.get()
    }
}

#[async_trait]
impl<'c> BackendTransaction<'c> for PgTransaction<'c> {
    async fn commit(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.commit().await?),
        }
    }

    async fn rollback(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.rollback().await?),
        }
    }
    // Workaround for https://github.com/rust-lang/rfcs/issues/2765
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

impl postgres::types::ToSql for SqlVal {
    fn to_sql(
        &self,
        ty: &postgres::types::Type,
        out: &mut bytes::BytesMut,
    ) -> std::result::Result<
        postgres::types::IsNull,
        Box<dyn std::error::Error + 'static + Sync + Send>,
    > {
        self.as_ref().to_sql(ty, out)
    }
    fn accepts(ty: &postgres::types::Type) -> bool {
        SqlValRef::<'_>::accepts(ty)
    }
    postgres::types::to_sql_checked!();
}

impl postgres::types::ToSql for SqlValRef<'_> {
    fn to_sql(
        &self,
        requested_ty: &postgres::types::Type,
        out: &mut bytes::BytesMut,
    ) -> std::result::Result<
        postgres::types::IsNull,
        Box<dyn std::error::Error + 'static + Sync + Send>,
    > {
        use SqlValRef::*;
        match self {
            Bool(b) => b.to_sql_checked(requested_ty, out),
            Int(i) => i.to_sql_checked(requested_ty, out),
            BigInt(i) => i.to_sql_checked(requested_ty, out),
            Real(r) => r.to_sql_checked(requested_ty, out),
            Text(t) => t.to_sql_checked(requested_ty, out),
            Blob(b) => b.to_sql_checked(requested_ty, out),
            #[cfg(feature = "json")]
            Json(v) => v.to_sql_checked(requested_ty, out),
            #[cfg(feature = "datetime")]
            Date(v) => v.to_sql_checked(requested_ty, out),
            #[cfg(feature = "datetime")]
            Timestamp(dt) => dt.to_sql_checked(requested_ty, out),
            Null => Ok(postgres::types::IsNull::Yes),
            Custom(SqlValRefCustom::PgToSql { ty, tosql }) => {
                check_type_match(ty, requested_ty)?;
                tosql.to_sql_checked(requested_ty, out)
            }
            Custom(SqlValRefCustom::PgBytes { ty, data }) => {
                check_type_match(ty, requested_ty)?;
                out.put(*data);
                Ok(postgres::types::IsNull::No)
            }
        }
    }
    fn accepts(_ty: &postgres::types::Type) -> bool {
        // Unfortunately this is a type method rather than an instance
        // method.  Declare acceptance of all the types we can support
        // and do the actual checking in the to_sql method
        true
    }
    postgres::types::to_sql_checked!();
}

fn check_type_match(
    ty1: &postgres::types::Type,
    ty2: &postgres::types::Type,
) -> std::result::Result<(), Box<dyn std::error::Error + 'static + Sync + Send>> {
    if ty1 == ty2 {
        Ok(())
    } else {
        Err(Box::new(crate::Error::Internal(format!(
            "postgres type mismatch. Wanted {ty1} but have {ty2}"
        ))))
    }
}

impl<'a> postgres::types::FromSql<'a> for SqlValRef<'a> {
    fn from_sql(
        ty: &postgres::types::Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + 'static + Sync + Send>> {
        use postgres::types::Type;
        match *ty {
            Type::BOOL => Ok(SqlValRef::Bool(bool::from_sql(ty, raw)?)),
            Type::INT4 => Ok(SqlValRef::Int(i32::from_sql(ty, raw)?)),
            Type::INT8 => Ok(SqlValRef::BigInt(i64::from_sql(ty, raw)?)),
            Type::FLOAT8 => Ok(SqlValRef::Real(f64::from_sql(ty, raw)?)),
            Type::TEXT => Ok(SqlValRef::Text(postgres::types::FromSql::from_sql(
                ty, raw,
            )?)),
            Type::BYTEA => Ok(SqlValRef::Blob(postgres::types::FromSql::from_sql(
                ty, raw,
            )?)),
            #[cfg(feature = "json")]
            Type::JSONB => Ok(SqlValRef::Json(postgres::types::FromSql::from_sql(
                ty, raw,
            )?)),
            #[cfg(feature = "datetime")]
            Type::DATE => Ok(SqlValRef::Date(NaiveDate::from_sql(ty, raw)?)),
            #[cfg(feature = "datetime")]
            Type::TIMESTAMP => Ok(SqlValRef::Timestamp(NaiveDateTime::from_sql(ty, raw)?)),
            _ => Ok(SqlValRef::Custom(SqlValRefCustom::PgBytes {
                ty: ty.clone(),
                data: raw,
            })),
        }
    }

    fn from_sql_null(
        _ty: &postgres::types::Type,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + 'static + Sync + Send>> {
        Ok(SqlValRef::Null)
    }

    #[allow(clippy::match_like_matches_macro)]
    fn accepts(_ty: &postgres::types::Type) -> bool {
        // Unfortunately this is a type method rather than an instance
        // method, so we don't actually know what we can
        // support. Declare acceptance of all and do any actual type
        // checking in from_sql.
        true
    }
}

fn check_columns(row: &postgres::Row, cols: &[Column]) -> Result<()> {
    if cols.len() != row.len() {
        Err(Error::Internal(format!(
            "postgres returns columns {} doesn't match requested columns {}",
            row.len(),
            cols.len()
        )))
    } else {
        Ok(())
    }
}

impl BackendRow for postgres::Row {
    fn get(&self, idx: usize, _ty: SqlType) -> Result<SqlValRef<'_>> {
        Ok(self.try_get(idx)?)
    }
    fn len(&self) -> usize {
        postgres::Row::len(self)
    }
}

fn sql_for_expr<W>(
    expr: query::Expr,
    values: &mut Vec<SqlVal>,
    pls: &mut PgPlaceholderSource,
    w: &mut W,
) where
    W: Write,
{
    helper::sql_for_expr(expr, sql_for_expr, values, pls, w)
}

fn sql_val_from_postgres<I>(row: &postgres::Row, idx: I, col: &Column) -> Result<SqlVal>
where
    I: postgres::row::RowIndex + std::fmt::Display,
{
    let sqlref: SqlValRef = row.try_get(idx)?;
    let sqlval: SqlVal = sqlref.into();
    if sqlval.is_compatible(col.ty(), true) {
        Ok(sqlval)
    } else {
        Err(Error::SqlResultTypeMismatch {
            col: col.name().to_string(),
            detail: format!(
                "{:?} is not compatible with expected column type {}",
                &sqlval,
                col.ty()
            ),
        })
    }
}

fn sql_for_op(current: &mut ADB, op: &Operation) -> Result<String> {
    match op {
        Operation::DisableConstraints => Ok("".to_string()), // PostgreSQL doesn't need this
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
        Operation::EnableConstraints => Ok("".to_string()), // PostgreSQL doesn't need this
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
        helper::quote_reserved_word(&table.name),
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
            helper::quote_reserved_word(col.name()),
            col_sqltype(col)?,
        ));
    }
    Ok(format!(
        "{} {} {}",
        helper::quote_reserved_word(col.name()),
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
            format!(
                "ALTER TABLE {} ADD FOREIGN KEY ({}) REFERENCES {}({});",
                helper::quote_reserved_word(table_name),
                helper::quote_reserved_word(column.name()),
                helper::quote_reserved_word(literal.table_name()),
                helper::quote_reserved_word(literal.column_name()),
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
                    SqlType::Int => Ok(Cow::Borrowed("SERIAL")),
                    SqlType::BigInt => Ok(Cow::Borrowed("BIGSERIAL")),
                    _ => Err(Error::InvalidAuto(col.name().to_string())),
                }
            } else {
                Ok(match ty {
                    SqlType::Bool => Cow::Borrowed("BOOLEAN"),
                    SqlType::Int => Cow::Borrowed("INTEGER"),
                    SqlType::BigInt => Cow::Borrowed("BIGINT"),
                    SqlType::Real => Cow::Borrowed("DOUBLE PRECISION"),
                    SqlType::Text => Cow::Borrowed("TEXT"),
                    #[cfg(feature = "datetime")]
                    SqlType::Date => Cow::Borrowed("DATE"),
                    #[cfg(feature = "datetime")]
                    SqlType::Timestamp => Cow::Borrowed("TIMESTAMP"),
                    SqlType::Blob => Cow::Borrowed("BYTEA"),
                    #[cfg(feature = "json")]
                    SqlType::Json => Cow::Borrowed("JSONB"),
                    SqlType::Custom(c) => match c {
                        SqlTypeCustom::Pg(ref ty) => Cow::Owned(ty.name().to_string()),
                    },
                })
            }
        }
    }
}

fn drop_table(name: &str) -> String {
    format!("DROP TABLE {};", helper::quote_reserved_word(name))
}

fn add_column(tbl_name: &str, col: &AColumn) -> Result<String> {
    let default: SqlVal = helper::column_default(col)?;
    let mut stmts = vec![format!(
        "ALTER TABLE {} ADD COLUMN {} DEFAULT {};",
        helper::quote_reserved_word(tbl_name),
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
        helper::quote_reserved_word(tbl_name),
        helper::quote_reserved_word(name)
    )
}

fn change_column(table: &ATable, old: &AColumn, new: &AColumn) -> Result<String> {
    use helper::quote_reserved_word;
    let tbl_name = &table.name;

    // Let's figure out what changed about the column
    let mut stmts: Vec<String> = Vec::new();
    if old.name() != new.name() {
        // column rename
        stmts.push(format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {};",
            quote_reserved_word(tbl_name),
            quote_reserved_word(old.name()),
            quote_reserved_word(new.name())
        ));
    }
    if old.typeid()? != new.typeid()? {
        // column type change
        stmts.push(format!(
            "ALTER TABLE {} ALTER COLUMN {} SET DATA TYPE {};",
            quote_reserved_word(tbl_name),
            quote_reserved_word(old.name()),
            col_sqltype(new)?,
        ));
    }
    if old.nullable() != new.nullable() {
        stmts.push(format!(
            "ALTER TABLE {} ALTER COLUMN {} {} NOT NULL;",
            quote_reserved_word(tbl_name),
            quote_reserved_word(old.name()),
            if new.nullable() { "DROP" } else { "SET" }
        ));
    }
    if old.is_pk() != new.is_pk() {
        // Change to primary key
        // Either way, drop the previous primary key
        // Butane does not currently support composite primary keys

        if new.is_pk() {
            // Drop the old primary key
            stmts.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}_pkey;",
                quote_reserved_word(tbl_name),
                tbl_name
            ));

            // add the new primary key
            stmts.push(format!(
                "ALTER TABLE {} ADD PRIMARY KEY ({});",
                quote_reserved_word(tbl_name),
                quote_reserved_word(new.name())
            ));
        } else {
            // this field is no longer the primary key. Butane requires a single primary key,
            // so some other column must be the primary key now. It will drop the constraint when processed.
        }
    }
    if old.unique() != new.unique() {
        // Changed uniqueness constraint
        if new.unique() {
            stmts.push(format!(
                "ALTER TABLE {} ADD UNIQUE ({});",
                quote_reserved_word(tbl_name),
                quote_reserved_word(new.name())
            ));
        } else {
            // Standard constraint naming scheme
            stmts.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT {}_{}_key;",
                quote_reserved_word(tbl_name),
                tbl_name,
                &old.name()
            ));
        }
    }

    if old.default() != new.default() {
        stmts.push(match new.default() {
            None => format!(
                "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                quote_reserved_word(tbl_name),
                quote_reserved_word(old.name())
            ),
            Some(val) => format!(
                "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                quote_reserved_word(tbl_name),
                quote_reserved_word(old.name()),
                helper::sql_literal_value(val)?
            ),
        });
    }

    if old.reference() != new.reference() {
        if old.reference().is_some() {
            // Drop the old reference
            stmts.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT {}_{}_fkey;",
                quote_reserved_word(tbl_name),
                tbl_name,
                old.name()
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
    write!(w, "INTO {} (", helper::quote_reserved_word(table)).unwrap();
    helper::list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold(1, |n, _| {
        let sep = if n == 1 { "" } else { ", " };
        write!(w, "{sep}${n}").unwrap();
        n + 1
    });
    write!(w, ")").unwrap();
    write!(w, " ON CONFLICT ({}) DO ", pkcol.name()).unwrap();
    if columns.len() > 1 {
        write!(w, "UPDATE SET (").unwrap();
        helper::list_columns(columns, w);
        write!(w, ") = (").unwrap();
        columns.iter().fold("", |sep, c| {
            write!(w, "{}excluded.{}", sep, c.name()).unwrap();
            ", "
        });
        write!(w, ")").unwrap();
    } else {
        // If the pk is the only column and it already exists, then there's nothing to update.
        write!(w, "NOTHING").unwrap();
    }
}

fn pgtype_for_val(val: &SqlVal) -> postgres::types::Type {
    use postgres::types::Type;
    match val.sqltype() {
        None => Type::UNKNOWN,
        Some(SqlType::Bool) => postgres::types::Type::BOOL,
        Some(SqlType::Int) => postgres::types::Type::INT4,
        Some(SqlType::BigInt) => postgres::types::Type::INT8,
        Some(SqlType::Real) => postgres::types::Type::FLOAT8,
        Some(SqlType::Text) => postgres::types::Type::TEXT,
        Some(SqlType::Blob) => postgres::types::Type::BYTEA,
        #[cfg(feature = "json")]
        Some(SqlType::Json) => postgres::types::Type::JSON,
        #[cfg(feature = "datetime")]
        Some(SqlType::Date) => postgres::types::Type::DATE,
        #[cfg(feature = "datetime")]
        Some(SqlType::Timestamp) => postgres::types::Type::TIMESTAMP,
        Some(SqlType::Custom(inner)) => match inner {
            #[cfg(feature = "pg")]
            SqlTypeCustom::Pg(ty, ..) => ty,
        },
    }
}

#[derive(Debug)]
struct PgPlaceholderSource {
    n: i8,
}
impl PgPlaceholderSource {
    fn new() -> Self {
        PgPlaceholderSource { n: 1 }
    }
}
impl helper::PlaceholderSource for PgPlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<'_, str> {
        let ret = Cow::Owned(format!("${}", self.n));
        self.n += 1;
        ret
    }
}
