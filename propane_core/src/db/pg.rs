//! Postgresql database backend
use super::helper;
use super::*;
use crate::migrations::adb::{AColumn, ATable, Operation, ADB};
use crate::query;
use crate::{Result, SqlType, SqlVal};
use bytes;
#[cfg(feature = "datetime")]
use chrono::NaiveDateTime;
#[cfg(feature = "datetime")]
use log::warn;
use postgres;
use postgres::fallible_iterator::FallibleIterator;
use std::cell::RefCell;
use std::fmt::Write;

#[cfg(feature = "debug")]
use exec_time::exec_time;

use crate::connection_method_wrapper;

/// Pg [Backend][crate::db::Backend] implementation.
#[derive(Default)]
pub struct PgBackend {}
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
        "sqlite"
    }

    fn create_migration_sql(&self, current: &ADB, ops: &[Operation]) -> Result<String> {
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
    conn: RefCell<postgres::Client>,
}
impl PgConnection {
    fn open(params: &str) -> Result<Self> {
        Ok(PgConnection {
            // TODO add TLS support
            conn: RefCell::new(postgres::Client::connect(params, postgres::NoTls)?),
        })
    }
    // For use with the connection_method_wrapper macro
    fn wrapped_connection_methods<'a>(&'a self) -> Result<PgGenericClient<'a, postgres::Client>> {
        Ok(PgGenericClient { client: &self.conn })
    }
}
connection_method_wrapper!(PgConnection);
impl BackendConnection for PgConnection {
    fn transaction<'c>(&'c mut self) -> Result<Transaction<'c>> {
        let trans: postgres::Transaction<'_> = self.conn.get_mut().transaction()?;
        let trans = Box::new(PgTransaction::new(trans));
        Ok(Transaction::new(trans))
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(PgBackend {})
    }
    fn backend_name(&self) -> &'static str {
        "pg"
    }
}

struct PgGenericClient<'a, T>
where
    T: postgres::GenericClient,
{
    client: &'a RefCell<T>,
}

type DynToSqlPg = (dyn postgres::types::ToSql + Sync);

fn sqlval_for_pg_query(v: &SqlVal) -> &dyn postgres::types::ToSql {
    v as &dyn postgres::types::ToSql
}

impl<T> ConnectionMethods for PgGenericClient<'_, T>
where
    T: postgres::GenericClient,
{
    fn execute(&self, sql: &str) -> Result<()> {
        if cfg!(feature = "debug") {
            eprintln!("execute sql {}", sql);
        }
        self.client.borrow_mut().batch_execute(sql.as_ref())?;
        Ok(())
    }

    #[cfg_attr(feature = "debug", exec_time)]
    fn query(
        &self,
        table: &'static str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
    ) -> Result<RawQueryResult> {
        let mut sqlquery = String::new();
        helper::sql_select(columns, table, &mut sqlquery);
        let mut values: Vec<SqlVal> = Vec::new();
        if let Some(expr) = expr {
            sqlquery.write_str(" WHERE ").unwrap();
            sql_for_expr(
                query::Expr::Condition(Box::new(expr)),
                &mut values,
                &mut sqlquery,
            );
        }
        if let Some(limit) = limit {
            helper::sql_limit(limit, &mut sqlquery)
        }
        if cfg!(feature = "debug") {
            eprintln!("query sql {}", sqlquery);
        }

        let stmt = self.client.try_borrow_mut()?.prepare(&sqlquery)?;
        self.client
            .try_borrow_mut()?
            .query_raw(&stmt, values.iter().map(sqlval_for_pg_query))?
            .map_err(|e| Error::Postgres(e))
            .map(|r| row_from_postgres(&r, columns))
            .collect()
    }
    fn insert(
        &self,
        table: &'static str,
        columns: &[Column],
        pkcol: Column,
        values: &[SqlVal],
    ) -> Result<SqlVal> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(table, columns, false, &mut sql);
        write!(&mut sql, " RETURNING {}", pkcol.name()).unwrap();
        if cfg!(feature = "debug") {
            eprintln!("insert sql {}", sql);
        }

        // use query instead of execute so we can get our result back
        let pk: Option<SqlVal> = self
            .client
            .try_borrow_mut()?
            .query_raw(sql.as_str(), values.iter().map(sqlval_for_pg_query))?
            .map_err(|e| Error::Postgres(e))
            .map(|r| sql_val_from_postgres(&r, 0, &pkcol))
            .nth(0)?;
        pk.ok_or(Error::Internal)
    }
    fn insert_or_replace(
        &self,
        table: &'static str,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_insert_with_placeholders(table, columns, true, &mut sql);
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        self.client
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
        Ok(())
    }
    fn update(
        &self,
        table: &'static str,
        pkcol: Column,
        pk: SqlVal,
        columns: &[Column],
        values: &[SqlVal],
    ) -> Result<()> {
        let mut sql = String::new();
        helper::sql_update_with_placeholders(table, pkcol, columns, &mut sql);
        let placeholder_values = [values, &[pk]].concat();
        let params: Vec<&DynToSqlPg> = placeholder_values
            .iter()
            .map(|v| v as &DynToSqlPg)
            .collect();
        if cfg!(feature = "debug") {
            eprintln!("update sql {}", sql);
        }
        self.client
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
        Ok(())
    }
    fn delete_where(&self, table: &'static str, expr: BoolExpr) -> Result<usize> {
        let mut sql = String::new();
        let mut values: Vec<SqlVal> = Vec::new();
        write!(&mut sql, "DELETE FROM {} WHERE ", table).unwrap();
        sql_for_expr(
            query::Expr::Condition(Box::new(expr)),
            &mut values,
            &mut sql,
        );
        let params: Vec<&DynToSqlPg> = values.iter().map(|v| v as &DynToSqlPg).collect();
        let cnt = self
            .client
            .try_borrow_mut()?
            .execute(sql.as_str(), params.as_slice())?;
        Ok(cnt as usize)
    }
    fn has_table(&self, table: &'static str) -> Result<bool> {
        // future improvement, should be schema-aware
        let stmt = self
            .client
            .try_borrow_mut()?
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_name=?;")?;
        let rows = self.client.try_borrow_mut()?.query(&stmt, &[&table])?;
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
            None => Err(Error::Internal),
            Some(trans) => Ok(trans),
        }
    }
    fn wrapped_connection_methods(&self) -> Result<PgGenericClient<'_, postgres::Transaction<'c>>> {
        Ok(PgGenericClient {
            client: self.get()?,
        })
    }
}
connection_method_wrapper!(PgTransaction<'_>);

impl<'c> BackendTransaction<'c> for PgTransaction<'c> {
    fn commit(&mut self) -> Result<()> {
        match self.trans.take() {
            None => Err(Error::Internal),
            Some(trans) => Ok(trans.into_inner().commit()?),
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
        match self {
            SqlVal::Bool(b) => b.to_sql(ty, out),
            SqlVal::Int(i) => i.to_sql(ty, out),
            SqlVal::Real(r) => r.to_sql(ty, out),
            SqlVal::Text(t) => t.to_sql(ty, out),
            SqlVal::Blob(b) => b.to_sql(ty, out),
            #[cfg(feature = "datetime")]
            SqlVal::Timestamp(dt) => dt.to_sql(ty, out),
            SqlVal::Null => Option::<bool>::None.to_sql(ty, out),
        }
    }

    fn accepts(ty: &postgres::types::Type) -> bool {
        // Unfortunately this is a type method rather than an instance method.
        // Declare acceptance of all the types we can support and hope it works out OK
        if bool::accepts(ty)
            || i32::accepts(ty)
            || i64::accepts(ty)
            || f64::accepts(ty)
            || String::accepts(ty)
            || Vec::<u8>::accepts(ty)
        {
            return true;
        }

        #[cfg(feature = "datetime")]
        if NaiveDateTime::accepts(ty) {
            return true;
        }

        return false;
    }

    postgres::types::to_sql_checked!();
}

impl<'a> postgres::types::FromSql<'a> for SqlVal {
    fn from_sql(
        ty: &postgres::types::Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + 'static + Sync + Send>> {
        use postgres::types::Type;
        match ty {
            &Type::BOOL => Ok(SqlVal::Bool(bool::from_sql(ty, raw)?)),
            &Type::INT2 => Ok(SqlVal::Int(i16::from_sql(ty, raw)? as i64)),
            &Type::INT4 => Ok(SqlVal::Int(i32::from_sql(ty, raw)? as i64)),
            &Type::INT8 => Ok(SqlVal::Int(i64::from_sql(ty, raw)?)),
            &Type::FLOAT4 => Ok(SqlVal::Real(f32::from_sql(ty, raw)? as f64)),
            &Type::FLOAT8 => Ok(SqlVal::Real(f64::from_sql(ty, raw)?)),
            &Type::TEXT => Ok(SqlVal::Text(String::from_sql(ty, raw)?)),
            &Type::BYTEA => Ok(SqlVal::Blob(Vec::<u8>::from_sql(ty, raw)?)),
            #[cfg(feature = "datetime")]
            &Type::TIMESTAMP => Ok(SqlVal::Timestamp(NaiveDateTime::from_sql(ty, raw)?)),
            _ => Err(Box::new(Error::UnknownSqlType(format!("{}", ty)))),
        }
    }

    fn accepts(ty: &postgres::types::Type) -> bool {
        use postgres::types::Type;
        match ty {
            &Type::BOOL => true,
            &Type::INT2 => true,
            &Type::INT4 => true,
            &Type::INT8 => true,
            &Type::FLOAT4 => true,
            &Type::FLOAT8 => true,
            &Type::TEXT => true,
            &Type::BYTEA => true,
            #[cfg(feature = "datetime")]
            &Type::TIMESTAMP => true,
            _ => false,
        }
    }
}

fn row_from_postgres(row: &postgres::Row, cols: &[Column]) -> Result<Row> {
    let mut vals: Vec<SqlVal> = Vec::new();
    if cols.len() != row.len() {
        panic!(
            "postgres returns columns {} doesn't match requested columns {}",
            row.len(),
            cols.len()
        )
    }
    vals.reserve(cols.len());
    for i in 0..cols.len() {
        let col = cols.get(i).unwrap();
        vals.push(sql_val_from_postgres(row, i, col)?);
    }
    Ok(Row::new(vals))
}

fn sql_for_expr<W>(expr: query::Expr, values: &mut Vec<SqlVal>, w: &mut W)
where
    W: Write,
{
    helper::sql_for_expr(expr, &sql_for_expr, values, w)
}

fn sql_val_from_postgres<I>(row: &postgres::Row, idx: I, col: &Column) -> Result<SqlVal>
where
    I: postgres::row::RowIndex + std::fmt::Display,
{
    let sqlval: SqlVal = row.try_get(idx)?;
    if !sqlval.is_compatible(col.ty(), true) {
        Ok(sqlval)
    } else {
        Err(Error::SqlResultTypeMismatch(col.name().to_string()))
    }
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
    format!(
        "{} {} {}",
        &col.name(),
        col_sqltype(col),
        constraints.join(" ")
    )
}

fn col_sqltype(col: &AColumn) -> &'static str {
    match col.sqltype() {
        Ok(ty) => sqltype(ty),
        // sqlite doesn't actually require that the column type be
        // specified
        Err(_) => "",
    }
}

fn sqltype(ty: SqlType) -> &'static str {
    match ty {
        SqlType::Bool => "INTEGER",
        SqlType::Int => "INTEGER",
        SqlType::BigInt => "INTEGER",
        SqlType::Real => "REAL",
        SqlType::Text => "TEXT",
        #[cfg(feature = "datetime")]
        SqlType::Timestamp => "TEXT",
        SqlType::Blob => "BLOB",
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
        helper::sql_literal_value(default)
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
            warn!(
                "Cannot remove column {} that does not exist from table {}",
                name, tbl_name
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
    format!("{}__propane_tmp", name)
}

fn change_column(
    current: &mut ADB,
    tbl_name: &str,
    old: &AColumn,
    new: Option<&AColumn>,
) -> String {
    let table = current.get_table(tbl_name);
    if table.is_none() {
        warn!(
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
