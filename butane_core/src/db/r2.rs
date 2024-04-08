//! R2D2 support for Butane.

pub use r2d2::ManageConnection;

use crate::connection_method_wrapper;
use crate::db::{
    BackendConnection, Column, Connection, ConnectionMethods, ConnectionSpec, RawQueryResult,
};
use crate::{query::BoolExpr, Result, SqlVal, SqlValRef};

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

connection_method_wrapper!(r2d2::PooledConnection<ConnectionManager>);
