//! Postgresql database backend
use super::connmethods::VecRows;
use super::helper;
use super::*;
use crate::custom::{SqlTypeCustom, SqlValRefCustom};
use crate::migrations::adb::{AColumn, ATable, Operation, TypeIdentifier, ADB};
use crate::{debug, query, warn};
use crate::{Result, SqlType, SqlVal, SqlValRef};
use async_trait::async_trait;
use bytes::BufMut;
#[cfg(feature = "datetime")]
use chrono::NaiveDateTime;
use futures_util::StreamExt;
use std::fmt::Write;
use tokio;
use tokio_postgres as postgres;
use tokio_postgres::GenericClient;

/// The name of the postgres backend.
pub const BACKEND_NAME: &str = "pg";

/// Pg [Backend][crate::db::Backend] implementation.
#[derive(Default)]
pub struct PgBackend {}
impl PgBackend {
    pub fn new() -> PgBackend {
        PgBackend {}
    }
}
impl PgBackend {
    async fn connect(&self, params: &str) -> Result<PgConnection> {
        PgConnection::open(params).await
    }
}

#[async_trait]
impl Backend for PgBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn create_migration_sql(&self, current: &ADB, ops: Vec<Operation>) -> Result<String> {
        let mut current: ADB = (*current).clone();
        Ok(ops
            .iter()
            .map(|o| sql_for_op(&mut current, o))
            .collect::<Result<Vec<String>>>()?
            .join("\n"))
    }

    async fn connect(&self, path: &str) -> Result<Connection> {
        Ok(Connection {
            conn: Box::new(self.connect(path).await?),
        })
    }
}

/// Pg database connection.
pub struct PgConnection {
    client: postgres::Client,
    // Save the handle to the task running the connection to keep it alive
    conn_handle: tokio::task::JoinHandle<()>,
}

impl PgConnection {
    async fn open(params: &str) -> Result<Self> {
        let (client, conn_handle) = Self::connect(params).await?;
        Ok(Self {
            client,
            conn_handle,
        })
    }
    async fn connect(params: &str) -> Result<(postgres::Client, tokio::task::JoinHandle<()>)> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "tls")] {
                let connector = native_tls::TlsConnector::new()?;
                let connector = postgres_native_tls::MakeTlsConnector::new(connector);
            } else {
                let connector = postgres::NoTls;
            }
        }
        let (client, conn) = postgres::connect(params, connector).await?;
        let conn_handle = tokio::spawn(async move {
            if let Err(e) = conn.await {
                warn!("Postgres connection error {}", e);
            }
        });
        Ok((client, conn_handle))
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

type DynToSqlPg<'a> = (dyn postgres::types::ToSql + Sync + 'a);

fn sqlval_for_pg_query(v: &SqlVal) -> &dyn postgres::types::ToSql {
    v as &dyn postgres::types::ToSql
}

fn sqlvalref_for_pg_query<'a>(v: &'a SqlValRef<'a>) -> &'a dyn postgres::types::ToSql {
    v as &dyn postgres::types::ToSql
}

/// Shared functionality between connection and
/// transaction. Implementation detail. Semver exempt.
pub trait PgConnectionLike {
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
            debug!("execute sql {}", sql);
        }
        self.client()?.batch_execute(sql.as_ref()).await?;
        Ok(())
    }

    async fn query<'a, 'b, 'c: 'a>(
        &'c self,
        table: &str,
        columns: &'b [Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        order: Option<&[query::Order]>,
    ) -> Result<RawQueryResult<'a>> {
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
            debug!("query sql {}", sqlquery);
        }
        eprintln!("query sql {}", sqlquery);

        let types: Vec<postgres::types::Type> = values.iter().map(pgtype_for_val).collect();
        let stmt = self
            .client()?
            .prepare_typed(&sqlquery, types.as_ref())
            .await?;
        // todo avoid intermediate vec?
        let rowvec = self
            .client()?
            .query_raw(&stmt, values.iter().map(sqlval_for_pg_query))
            .await
            .map_err(Error::Postgres)
            .map(|r| {
                check_columns(&r, columns)?;
                Ok(r)
            })??;
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
            debug!("insert sql {}", sql);
        }

        // use query instead of execute so we can get our result back
        let pk: Option<SqlVal> = self
            .client()?
            .query_raw(sql.as_str(), values.iter().map(sqlvalref_for_pg_query))
            .await
            .map_err(Error::Postgres)
            .map(|r| sql_val_from_postgres(&r, 0, pkcol))??
            .nth(0)?;
        pk.ok_or_else(|| Error::Internal("could not get pk".to_string()))
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
        self.client()?
            .execute(sql.as_str(), params.as_slice())
            .await?;
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
        self.client()?
            .execute(sql.as_str(), params.as_slice())
            .await?;
        Ok(())
    }
    async fn update(
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let placeholder_values = [values, &[pk]].concat();
        let params: Vec<&DynToSqlPg> = placeholder_values
            .iter()
            .map(|v| v as &DynToSqlPg)
            .collect();
        if cfg!(feature = "log") {
            debug!("update sql {}", sql);
        }
        self.client()?
            .execute(sql.as_str(), params.as_slice())
            .await?;
        Ok(())
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(&mut sql, "DELETE FROM {} WHERE ", table).unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let cnt = self
            .client()?
            .execute(sql.as_str(), params.as_slice())
            .await?;
        Ok(cnt as usize)
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        // future improvement, should be schema-aware
        let stmt = self
            .client()?
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_name=$1;")
            .await?;
        let rows = self.client()?.query(&stmt, &[&table]).await?;
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
        match self.trans {
            Some(x) => Ok(&x),
            None => Err(Self::already_consumed()),
        }
    }
    fn already_consumed() -> Error {
        Error::Internal("transaction has already been consumed".to_string())
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
    fn connection_methods_mut(&mut self) -> &mut dyn ConnectionMethods {
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

impl<'a> postgres::types::ToSql for SqlValRef<'a> {
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
            "postgres type mismatch. Wanted {} but have {}",
            ty1, ty2
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

fn check_columns(row: &postgres::RowStream, cols: &[Column]) -> Result<()> {
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
    fn get(&self, idx: usize, _ty: SqlType) -> Result<SqlValRef> {
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

fn sql_val_from_postgres<I>(row: &postgres::RowStream, idx: I, col: &Column) -> Result<SqlVal>
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
        Operation::AddTable(table) => Ok(create_table(table, false)?),
        Operation::AddTableIfNotExists(table) => Ok(create_table(table, true)?),
        Operation::RemoveTable(name) => Ok(drop_table(name)),
        Operation::AddColumn(tbl, col) => add_column(tbl, col),
        Operation::RemoveColumn(tbl, name) => Ok(remove_column(tbl, name)),
        Operation::ChangeColumn(tbl, old, new) => change_column(current, tbl, old, Some(new)),
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
        modifier, table.name, coldefs
    ))
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
    Ok(format!(
        "{} {} {}",
        &col.name(),
        col_sqltype(col)?,
        constraints.join(" ")
    ))
}

fn col_sqltype(col: &AColumn) -> Result<Cow<str>> {
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
                    SqlType::Timestamp => Cow::Borrowed("TIMESTAMP"),
                    SqlType::Blob => Cow::Borrowed("BYTEA"),
                    SqlType::Custom(c) => match c {
                        SqlTypeCustom::Pg(ref ty) => Cow::Owned(ty.name().to_string()),
                    },
                })
            }
        }
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
        define_column(col)?,
        helper::sql_literal_value(default)?
    ))
}

fn remove_column(tbl_name: &str, name: &str) -> String {
    format!("ALTER TABLE {} DROP COLUMN {};", tbl_name, name)
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
) -> Result<String> {
    let table = current.get_table(tbl_name);
    if table.is_none() {
        crate::warn!(
            "Cannot alter column {} from table {} that does not exist",
            &old.name(),
            tbl_name
        );
        return Ok(String::new());
    }
    let old_table = table.unwrap();
    let mut new_table = old_table.clone();
    new_table.name = tmp_table_name(&new_table.name);
    match new {
        Some(col) => new_table.replace_column(col.clone()),
        None => new_table.remove_column(old.name()),
    }
    let stmts: [&str; 4] = [
        &create_table(&new_table, false)?,
        &copy_table(old_table, &new_table),
        &drop_table(&old_table.name),
        &format!("ALTER TABLE {} RENAME TO {};", &new_table.name, tbl_name),
    ];
    let result = stmts.join("\n");
    new_table.name = old_table.name.clone();
    current.replace_table(new_table);
    Ok(result)
}

pub fn sql_insert_or_replace_with_placeholders(
    table: &str,
    columns: &[Column],
    pkcol: &Column,
    w: &mut impl Write,
) {
    write!(w, "INSERT ").unwrap();
    write!(w, "INTO {} (", table).unwrap();
    helper::list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold(1, |n, _| {
        let sep = if n == 1 { "" } else { ", " };
        write!(w, "{}${}", sep, n).unwrap();
        n + 1
    });
    write!(w, ")").unwrap();
    write!(w, " ON CONFLICT ({}) DO UPDATE SET (", pkcol.name()).unwrap();
    helper::list_columns(columns, w);
    write!(w, ") = (").unwrap();
    columns.iter().fold("", |sep, c| {
        write!(w, "{}excluded.{}", sep, c.name()).unwrap();
        ", "
    });
    write!(w, ")").unwrap();
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
        #[cfg(feature = "datetime")]
        Some(SqlType::Timestamp) => postgres::types::Type::TIMESTAMP,
        Some(SqlType::Custom(inner)) => match inner {
            #[cfg(feature = "pg")]
            SqlTypeCustom::Pg(ty, ..) => ty,
        },
    }
}

struct PgPlaceholderSource {
    n: i8,
}
impl PgPlaceholderSource {
    fn new() -> Self {
        PgPlaceholderSource { n: 1 }
    }
}
impl helper::PlaceholderSource for PgPlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<str> {
        let ret = Cow::Owned(format!("${}", self.n));
        self.n += 1;
        ret
    }
}
