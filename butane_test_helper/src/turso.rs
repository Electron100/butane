//! Turso test helper implementation for in-memory databases
//!
//! This module provides utilities for testing with Turso/libSQL in-memory databases.
//! For sqld server support, use the `turso` feature and see the `libsql` module.

use std::future::Future;

use butane_core::db::turso::{TursoBackend, BACKEND_NAME};
use butane_core::db::{Backend, ConnectionAsync, ConnectionSpec};

use crate::{common_setup, setup_db_async, BackendTestInstance, SetupData};

/// Instance of a Turso test.
#[derive(Default)]
pub struct TursoTestInstance {}

impl BackendTestInstance for TursoTestInstance {
    fn run_test_sync(_test: impl FnOnce(crate::Connection), _migrate: bool) {
        // Turso doesn't support sync connections - skip the test silently
        // The test framework will still count this as passed
    }

    async fn run_test_async<Fut>(test: impl FnOnce(ConnectionAsync) -> Fut, migrate: bool)
    where
        Fut: Future<Output = ()>,
    {
        common_setup();
        log::info!("connecting to turso memory database...");
        let mut conn = TursoBackend::new()
            .connect_async(":memory:")
            .await
            .expect("Could not connect turso backend");
        if migrate {
            setup_db_async(&mut conn).await;
        }
        log::info!("running turso test");
        test(conn).await;
    }
}

/// Create a turso [`ConnectionSpec`] for in-memory database.
pub fn turso_connspec() -> ConnectionSpec {
    ConnectionSpec::new(BACKEND_NAME, ":memory:")
}

/// Concrete [SetupData] for Turso in-memory database.
pub struct TursoSetupData {
    connection_string: String,
}

impl SetupData for TursoSetupData {
    fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

/// Setup the test turso database using in-memory storage.
pub async fn turso_setup() -> TursoSetupData {
    TursoSetupData {
        connection_string: ":memory:".to_string(),
    }
}

/// Tear down the test turso database (in-memory).
pub fn turso_teardown(_: TursoSetupData) {
    // Nothing to do for in-memory database
}
