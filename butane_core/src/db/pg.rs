//! Postgresql database backend
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt::{Debug, Write};

use bytes::BufMut;
#[cfg(feature = "datetime")]
use chrono::NaiveDateTime;
use postgres::fallible_iterator::FallibleIterator;
use postgres::GenericClient;

use super::connmethods::VecRows;
use super::helper;
use crate::custom::{SqlTypeCustom, SqlValRefCustom};
use crate::db::{
    Backend, BackendConnection, BackendRow, BackendTransaction, Column, Connection,
    ConnectionMethods, RawQueryResult, Transaction,
};
use crate::migrations::adb::{AColumn, ARef, ATable, Operation, TypeIdentifier, ADB};
use crate::{debug, query};
use crate::{query::BoolExpr, Error, Result, SqlType, SqlVal, SqlValRef};

/// The name of the postgres backend.
pub const BACKEND_NAME: &str = "pg";

/// Postgres [`Backend`] implementation.
#[derive(Debug, Default)]
pub struct PgBackend;
impl PgBackend {
    pub fn new() -> PgBackend {
        PgBackend {}
    }
}
impl PgBackend {
    fn connect(&self, params: &str) -> Result<PgConnection> {
        PgConnection::open(params)
    }
}
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

    fn connect(&self, path: &str) -> Result<Connection> {
        Ok(Connection {
            conn: Box::new(self.connect(path)?),
        })
    }
}

/// Pg database connection.
pub struct PgConnection {
    #[cfg(feature = "debug")]
    params: Box<str>,
    conn: RefCell<postgres::Client>,
}
impl PgConnection {
    fn open(params: &str) -> Result<Self> {
        Ok(PgConnection {
            #[cfg(feature = "debug")]
            params: params.into(),
            conn: RefCell::new(Self::connect(params)?),
        })
    }
    fn connect(params: &str) -> Result<postgres::Client> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "tls")] {
                let connector = native_tls::TlsConnector::new()?;
                let connector = postgres_native_tls::MakeTlsConnector::new(connector);
            } else {
                let connector = postgres::NoTls;
            }
        }
        Ok(postgres::Client::connect(params, connector)?)
    }
}
impl PgConnectionLike for PgConnection {
    type Client = postgres::Client;
    fn cell(&self) -> Result<&RefCell<Self::Client>> {
        Ok(&self.conn)
    }
}
impl BackendConnection for PgConnection {
    fn transaction(&mut self) -> Result<Transaction<'_>> {
        let trans: postgres::Transaction<'_> = self.conn.get_mut().transaction()?;
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
        self.conn.borrow().is_closed()
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
    type Client: postgres::GenericClient;
    fn cell(&self) -> Result<&RefCell<Self::Client>>;
}

impl<T> ConnectionMethods for T
where
    T: PgConnectionLike,
{
    fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "log") {
            debug!("execute sql {}", sql);
        }
        self.cell()?.try_borrow_mut()?.batch_execute(sql.as_ref())?;
        Ok(())
    }

    fn query<'a, 'b, 'c: 'a>(
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

        let types: Vec<postgres::types::Type> = values.iter().map(pgtype_for_val).collect();
        let stmt = self
            .cell()?
            .try_borrow_mut()?
            .prepare_typed(&sqlquery, types.as_ref())?;
        // todo avoid intermediate vec?
        let rowvec: Vec<postgres::Row> = self
            .cell()?
            .try_borrow_mut()?
            .query_raw(&stmt, values.iter().map(sqlval_for_pg_query))?
            .map_err(Error::Postgres)
            .map(|r| {
                check_columns(&r, columns)?;
                Ok(r)
            })
            .collect()?;
        Ok(Box::new(VecRows::new(rowvec)))
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        write!(&mut sql, " RETURNING {}", pkcol.name()).unwrap();
        if cfg!(feature = "log") {
            debug!("insert sql {}", sql);
        }

        // use query instead of execute so we can get our result back
        let pk: Option<SqlVal> = self
            .cell()?
            .try_borrow_mut()?
            .query_raw(sql.as_str(), values.iter().map(sqlvalref_for_pg_query))?
            .map_err(Error::Postgres)
            .map(|r| sql_val_from_postgres(&r, 0, pkcol))
            .nth(0)?;
        pk.ok_or_else(|| Error::Internal("could not get pk".to_string()))
    }
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(
            table,
            columns,
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        self.cell()?
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
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
        sql_insert_or_replace_with_placeholders(table, columns, pkcol, &mut sql);
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        self.cell()?
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
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
        self.cell()?
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
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
            &mut PgPlaceholderSource::new(),
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let cnt = self
            .cell()?
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
        Ok(cnt as usize)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        // future improvement, should be schema-aware
        let stmt = self
            .cell()?
            .try_borrow_mut()?
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_name=$1;")?;
        let rows = self.cell()?.try_borrow_mut()?.query(&stmt, &[&table])?;
        Ok(!rows.is_empty())
    }
}

struct PgTransaction<'c> {
    trans: Option<RefCell<postgres::Transaction<'c>>>,
}
impl<'c> PgTransaction<'c> {
    fn new(trans: postgres::Transaction<'c>) -> Self {
        PgTransaction {
            trans: Some(RefCell::new(trans)),
        }
    }
    fn get(&self) -> Result<&RefCell<postgres::Transaction<'c>>> {
        match &self.trans {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans),
        }
    }
    fn already_consumed() -> Error {
        Error::Internal("transaction has already been consumed".to_string())
    }
}
impl<'c> Debug for PgTransaction<'c> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgTransaction")
            // postgres::Transaction doesnt expose any internal state
            .field("trans", &self.trans.is_some())
            .finish()
    }
}

impl<'c> PgConnectionLike for PgTransaction<'c> {
    type Client = postgres::Transaction<'c>;
    fn cell(&self) -> Result<&RefCell<Self::Client>> {
        self.get()
    }
}

impl<'c> BackendTransaction<'c> for PgTransaction<'c> {
    fn commit(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.into_inner().commit()?),
        }
    }
    fn rollback(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Self::already_consumed()),
            Some(trans) => Ok(trans.into_inner().rollback()?),
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
            #[cfg(feature = "json")]
            Json(v) => v.to_sql_checked(requested_ty, out),
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
        Operation::AddTable(table) => Ok(create_table(table, false)?),
        Operation::AddTableConstraints(table) => Ok(create_table_constraints(table)),
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
        modifier,
        helper::quote_reserved_word(&table.name),
        coldefs
    ))
}

fn create_table_constraints(table: &ATable) -> String {
    table
        .columns
        .iter()
        .filter(|column| column.reference().is_some())
        .map(|column| define_constraint(&table.name, column))
        .collect::<Vec<String>>()
        .join("\n")
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
        helper::quote_reserved_word(col.name()),
        col_sqltype(col)?,
        constraints.join(" ")
    ))
}

fn define_constraint(table_name: &str, column: &AColumn) -> String {
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
        helper::sql_literal_value(default)?
    )];
    if col.reference().is_some() {
        stmts.push(define_constraint(tbl_name, col));
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
    let mut stmts: Vec<String> = vec![
        create_table(&new_table, false)?,
        create_table_constraints(&new_table),
        copy_table(old_table, &new_table),
        drop_table(&old_table.name),
        format!(
            "ALTER TABLE {} RENAME TO {};",
            helper::quote_reserved_word(&new_table.name),
            helper::quote_reserved_word(tbl_name)
        ),
    ];
    stmts.retain(|stmt| !stmt.is_empty());
    let result = stmts.join("\n");
    new_table.name.clone_from(&old_table.name);
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
    fn next_placeholder(&mut self) -> Cow<str> {
        let ret = Cow::Owned(format!("${}", self.n));
        self.n += 1;
        ret
    }
}
