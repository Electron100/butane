#![allow(unused)]
// TODO needs module comment

use async_trait::async_trait;

use super::connmethods::sync::ConnectionMethods as ConnectionMethodsSync;
use super::sync::Backend as BackendSync;
use super::sync::BackendConnection as BackendConnectionSync;
use super::sync::Connection as ConnectionSync;
use super::sync::Transaction as TransactionSync;
use super::Transaction as TransactionAsync;
use super::{
    connmethods::Column, Backend, BackendConnection, Connection, ConnectionMethods, RawQueryResult,
    Transaction,
};
use crate::migrations::adb;
use crate::query::{BoolExpr, Order};
use crate::{Error, Result, SqlVal, SqlValRef};
use BackendConnection as BackendConnectionAsync;

#[derive(Clone, Debug)]
struct DummyBackend {}

#[maybe_async_cfg::maybe(
    idents(
        Backend(sync = "BackendSync", async),
        Connection(sync = "ConnectionSync", async)
    ),
    keep_self,
    sync(),
    async()
)]
#[async_trait]
impl Backend for DummyBackend {
    fn name(&self) -> &'static str {
        "dummy"
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String> {
        Err(Error::PoisonedConnection)
    }
    async fn connect(&self, conn_str: &str) -> Result<Connection> {
        Err(Error::PoisonedConnection)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DummyConnection {}
impl DummyConnection {
    pub fn new() -> Self {
        Self {}
    }
}

#[maybe_async_cfg::maybe(
    idents(ConnectionMethods(sync = "ConnectionMethodsSync", async)),
    keep_self,
    sync(),
    async()
)]
#[async_trait(?Send)]
impl ConnectionMethods for DummyConnection {
    async fn execute(&self, sql: &str) -> Result<()> {
        Err(Error::PoisonedConnection)
    }
    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        Err(Error::PoisonedConnection)
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        Err(Error::PoisonedConnection)
    }
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        Err(Error::PoisonedConnection)
    }
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        Err(Error::PoisonedConnection)
    }
    async fn update(
        &self,
        table: &str,
        pkcol: Column,
        pk: SqlValRef<'_>,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        Err(Error::PoisonedConnection)
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        Err(Error::PoisonedConnection)
    }
    async fn has_table(&self, table: &str) -> Result<bool> {
        Err(Error::PoisonedConnection)
    }
}

#[maybe_async_cfg::maybe(
    idents(Backend(sync = "BackendSync", async), BackendConnection, Transaction),
    keep_self,
    sync(),
    async()
)]
#[async_trait(?Send)]
impl BackendConnection for DummyConnection {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        Err(Error::PoisonedConnection)
    }
    fn backend(&self) -> Box<dyn Backend> {
        Box::new(DummyBackend {})
    }
    fn backend_name(&self) -> &'static str {
        "dummy"
    }
    fn is_closed(&self) -> bool {
        true
    }
}
