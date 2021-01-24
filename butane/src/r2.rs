use crate::db;
use crate::db::BackendConnection;
use crate::Result;

/// R2D2 support for Butane. Implements [`r2d2::ManageConnection`].
pub struct ConnectionManager {
    spec: db::ConnectionSpec,
}
impl ConnectionManager {
    pub fn new(spec: db::ConnectionSpec) -> Self {
        ConnectionManager { spec }
    }
}

impl r2d2::ManageConnection for ConnectionManager {
    type Connection = db::Connection;
    type Error = crate::Error;

    fn connect(&self) -> Result<Self::Connection> {
        db::connect(&self.spec)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<()> {
        conn.execute("SELECT 1")
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.is_closed()
    }
}
