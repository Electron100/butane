//! R2D2 support for Butane.
use r2d2::ManageConnection;

use super::ConnectionManager;
use crate::db::{BackendConnection, Connection};
use crate::Result;

impl ManageConnection for ConnectionManager {
    type Connection = Connection;
    type Error = crate::Error;

    fn connect(&self) -> Result<Connection> {
        crate::db::connect(&self.spec)
    }

    fn is_valid(&self, conn: &mut Connection) -> Result<()> {
        conn.execute("SELECT 1")
    }

    fn has_broken(&self, conn: &mut Connection) -> bool {
        conn.is_closed()
    }
}
