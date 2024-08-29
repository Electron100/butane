//! R2D2 support for Butane.

pub use r2d2::ManageConnection;

use crate::db::{BackendConnection, Connection, ConnectionMethods};
use crate::db::{Column, ConnectionSpec, RawQueryResult};
use crate::{query::BoolExpr, query::Order, Result, SqlVal, SqlValRef};

use std::ops::Deref;

/// R2D2 support for Butane. Implements [`r2d2::ManageConnection`].
#[derive(Clone, Debug)]
pub struct ConnectionManager {
    spec: ConnectionSpec,
}
impl ConnectionManager {
    pub fn new(spec: ConnectionSpec) -> Self {
        ConnectionManager { spec }
    }
}

impl ManageConnection for ConnectionManager {
    type Connection = Connection;
    type Error = crate::Error;

    fn connect(&self) -> Result<Self::Connection> {
        crate::db::connect(&self.spec)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<()> {
        conn.execute("SELECT 1")
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.is_closed()
    }
}

impl ConnectionMethods for r2d2::PooledConnection<ConnectionManager> {
    fn execute(&self, sql: &str) -> Result<()> {
        self.deref().execute(sql)
    }
    fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        self.deref()
            .query(table, columns, expr, limit, offset, sort)
    }
    fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.deref()
            .insert_returning_pk(table, columns, pkcol, values)
    }
    /// Like `insert_returning_pk` but with no return value
    fn insert_only(&self, table: &str, columns: &[Column], values: &[SqlValRef<'_>]) -> Result<()> {
        self.deref().insert_only(table, columns, values)
    }
    /// Insert unless there's a conflict on the primary key column, in which case update
    fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.deref()
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
        self.deref().update(table, pkcol, pk, columns, values)
    }
    fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.deref().delete_where(table, expr)
    }
    fn has_table(&self, table: &str) -> Result<bool> {
        self.deref().has_table(table)
    }
}
