//! Deadpool support for Butane.
use super::ConnectionManager;
use crate::db::{BackendConnectionAsync, ConnectionAsync};
use crate::Result;
use deadpool::managed::{Manager, Metrics, RecycleError, RecycleResult};

impl Manager for ConnectionManager {
    type Type = ConnectionAsync;
    type Error = crate::Error;

    async fn create(&self) -> Result<ConnectionAsync> {
        crate::db::connect_async(&self.spec).await
    }

    async fn recycle(&self, conn: &mut ConnectionAsync, _: &Metrics) -> RecycleResult<Self::Error> {
        if conn.is_closed() {
            return Err(RecycleError::message("Connection is closed"));
        }
        Ok(())
    }
}
