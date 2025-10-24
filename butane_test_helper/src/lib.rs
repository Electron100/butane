//! Test helpers to set up database connections.
//! Macros depend on [`butane_core`], `env_logger` and [`log`].
//!
//! # Using ephemeralpg
//!
//! This crate supports using [ephemeralpg](https://github.com/eradman/ephemeralpg)
//! as an alternative to the built-in PostgreSQL server management.
//!
//! The library will automatically detect and use `pg_tmp` if available, otherwise
//! it will fall back to using `initdb` and `postgres` directly.
//!
//! To use ephemeralpg:
//! 1. Install ephemeralpg (see <https://github.com/eradman/ephemeralpg>)
//! 2. Ensure `pg_tmp` is in your PATH
//!
//! The library will automatically detect and prefer `pg_tmp` over `initdb`.
#![deny(missing_docs)]

extern crate alloc;

#[cfg(feature = "pg")]
mod pg;

use std::future::Future;
#[cfg(feature = "pg")]
use std::ops::Deref;
#[cfg(feature = "pg")]
use std::sync::{LazyLock, Mutex};

#[cfg(feature = "pg")]
use butane_core::db::pg::PgBackend;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite::SQLiteBackend;
#[cfg(feature = "pg")]
use butane_core::db::{connect, connect_async, pg as butane_pg};
use butane_core::db::{get_backend, Backend, ConnectionSpec};

use butane_core::migrations::{self, MemMigrations, Migration, Migrations, MigrationsMut};
#[cfg(feature = "pg")]
use uuid::Uuid;

// Re-export as they are used by the macros.
pub use butane_core::db::{BackendConnection, BackendConnectionAsync, Connection, ConnectionAsync};
pub use maybe_async_cfg;

/// Trait for running a test.
#[allow(async_fn_in_trait)] // Not truly public, only used in butane for testing.
pub trait BackendTestInstance {
    /// Run a synchronous test.
    fn run_test_sync(test: impl FnOnce(Connection), migrate: bool);
    /// Run an asynchronous test.
    async fn run_test_async<Fut>(test: impl FnOnce(ConnectionAsync) -> Fut, migrate: bool)
    where
        Fut: Future<Output = ()>;
}

/// Instance of a Postgres test.
#[cfg(feature = "pg")]
#[derive(Default)]
pub struct PgTestInstance {}

#[cfg(feature = "pg")]
impl BackendTestInstance for PgTestInstance {
    fn run_test_sync(test: impl FnOnce(Connection), migrate: bool) {
        common_setup();
        let backend = PgBackend::new();
        let setup_data = pg_setup_sync();
        let connstr = setup_data.connection_string();
        log::info!("connecting to {}..", connstr);
        let mut conn = backend.connect(connstr).expect("Could not connect backend");
        if migrate {
            setup_db(&mut conn);
        }
        log::info!("running test on {}...", connstr);
        test(conn);
    }
    async fn run_test_async<Fut>(test: impl FnOnce(ConnectionAsync) -> Fut, migrate: bool)
    where
        Fut: Future<Output = ()>,
    {
        common_setup();
        let backend = PgBackend::new();
        let setup_data = pg_setup().await;
        let connstr = setup_data.connection_string();
        log::info!("connecting to {}..", connstr);
        let mut conn = backend
            .connect_async(connstr)
            .await
            .expect("Could not connect pg backend");
        if migrate {
            setup_db_async(&mut conn).await;
        }
        log::info!("running test on {}...", connstr);
        test(conn).await;
    }
}

/// Instance of a SQLite test.
#[cfg(feature = "sqlite")]
#[derive(Default)]
pub struct SQLiteTestInstance {}

#[cfg(feature = "sqlite")]
impl BackendTestInstance for SQLiteTestInstance {
    fn run_test_sync(test: impl FnOnce(Connection), migrate: bool) {
        common_setup();
        log::info!("connecting to sqlite memory database..");
        let mut conn = SQLiteBackend::new()
            .connect(":memory:")
            .expect("Could not connect sqlite backend");
        if migrate {
            setup_db(&mut conn);
        }
        log::info!("running sqlite test");
        test(conn);
    }
    async fn run_test_async<Fut>(test: impl FnOnce(ConnectionAsync) -> Fut, migrate: bool)
    where
        Fut: Future<Output = ()>,
    {
        common_setup();
        log::info!("connecting to sqlite memory database...");
        let mut conn = SQLiteBackend::new()
            .connect_async(":memory:")
            .await
            .expect("Could not connect sqlite backend");
        if migrate {
            setup_db_async(&mut conn).await;
        }
        log::info!("running sqlite test");
        test(conn).await;
    }
}

/// Used with `run_test` and `run_test_async`. Result of a backend-specific setup function.
/// Provides a connection string, and also passed to the backend-specific teardown function.
pub trait SetupData {
    /// Return the connection string to use when establishing a
    /// database connection.
    fn connection_string(&self) -> &str;
}

/// Create a PostgreSQL [`Connection`].
#[cfg(feature = "pg")]
pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = get_backend(butane_pg::BACKEND_NAME).unwrap();
    let data = pg_setup_sync();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

/// Create a PostgreSQL [`ConnectionSpec`].
#[cfg(feature = "pg")]
pub async fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup().await;
    (
        ConnectionSpec::new(butane_pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

// Re-export types from pg module
#[cfg(feature = "pg")]
pub use pg::{
    cleanup_postgres_shared_memory, pg_tmp_server_create, pg_tmp_server_create_ephemeralpg,
    pg_tmp_server_create_using_initdb, PgServerOptions, PgServerState, PgTemporaryServerError,
};

// Backward compatibility alias for the old typo'd name
#[cfg(feature = "pg")]
pub use pg::PgTemporaryServerError as PgTemporaryServenError;

/// Connection spec for a test server.
#[cfg(feature = "pg")]
#[derive(Clone, Debug)]
pub struct PgSetupData {
    /// Connection string
    connection_string: String,
}
#[cfg(feature = "pg")]
impl SetupData for PgSetupData {
    fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

#[cfg(feature = "pg")]
/// Try to delete all the pg files when the process exits.
extern "C" fn pg_tmp_server_proc_teardown() {
    drop(TMP_SERVER.deref().lock().unwrap().take());
}

#[cfg(feature = "pg")]
static TMP_SERVER: LazyLock<Mutex<Option<PgServerState>>> = LazyLock::new(|| {
    Mutex::new(Some(
        pg_tmp_server_create(PgServerOptions {
            atexit_callback: Some(pg_tmp_server_proc_teardown),
            ..Default::default()
        })
        .unwrap(),
    ))
});

/// Create a running empty PostgreSQL database named `butane_test_<uuid>`.
#[cfg(feature = "pg")]
pub fn pg_setup_sync() -> PgSetupData {
    log::trace!("starting pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR
    let mut connection_spec = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(value) => ConnectionSpec::try_from(value).unwrap(),
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();

            // If using ephemeralpg, we need to connect to the default database
            // to create a new one for this test
            if let Some(ref uri) = server.ephemeralpg_uri {
                // Use the ephemeralpg URI as-is to connect to the default "test" database
                ConnectionSpec::try_from(uri.clone()).unwrap()
            } else {
                let host = server.sockdir.path().to_str().unwrap();
                ConnectionSpec::new("pg", format!("host={host} user=postgres"))
            }
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    log::info!("new db is `{}`", &new_dbname);

    let conn = connect(&connection_spec).unwrap();
    log::debug!("closed is {}", BackendConnection::is_closed(&conn));
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .unwrap();

    connection_spec
        .add_parameter("dbname", &new_dbname)
        .unwrap();
    PgSetupData {
        connection_string: connection_spec.connection_string().clone(),
    }
}

/// Create a running empty PostgreSQL database named `butane_test_<uuid>`.
#[cfg(feature = "pg")]
pub async fn pg_setup() -> PgSetupData {
    log::trace!("starting pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR (which must be a PostgreSQL KV-style
    // string, not a URL, e.g. "host=localhost user=postgres").
    let mut connection_spec = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(value) => ConnectionSpec::try_from(value).unwrap(),
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();

            // If using ephemeralpg, we need to connect to the default database
            // to create a new one for this test
            if let Some(ref uri) = server.ephemeralpg_uri {
                // Use the ephemeralpg URI as-is to connect to the default "test" database
                ConnectionSpec::try_from(uri.clone()).unwrap()
            } else {
                let host = server.sockdir.path().to_str().unwrap();
                ConnectionSpec::new("pg", format!("host={host} user=postgres"))
            }
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    log::info!("new db is `{}`", &new_dbname);

    let conn = connect_async(&connection_spec).await.unwrap();
    log::debug!(
        "[async]closed is {}",
        BackendConnectionAsync::is_closed(&conn)
    );
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .await
        .unwrap();

    connection_spec
        .add_parameter("dbname", &new_dbname)
        .unwrap();
    PgSetupData {
        connection_string: connection_spec.connection_string().clone(),
    }
}

/// Tear down PostgreSQL database created by [`pg_setup`].
#[cfg(feature = "pg")]
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

/// Obtain the connection string for the PostgreSQL database.
#[cfg(feature = "pg")]
pub fn pg_connstr(data: &PgSetupData) -> String {
    data.connection_string().to_string()
}

/// Create a [`MemMigrations`]` for the "current" migration.
pub fn create_current_migrations(backend: Box<dyn Backend>) -> MemMigrations {
    let mut root = std::env::current_dir().unwrap();
    root.push(".butane/migrations");
    let mut disk_migrations = migrations::from_root(&root);
    let disk_current = disk_migrations.current();
    log::info!("Loading migrations from {:?}", disk_current);
    // Create an in-memory Migrations and write only to that. This
    // allows concurrent tests to avoid stomping on each other and is
    // also faster than real disk writes.
    let mut mem_migrations = MemMigrations::new();
    let mem_current = mem_migrations.current();

    migrations::copy_migration(disk_current, mem_current).unwrap();

    assert!(
        disk_current.db().unwrap().tables().count() != 0,
        "No tables to migrate"
    );

    assert!(
        mem_migrations
            .create_migration(&nonempty::nonempty![backend], "init", None)
            .expect("expected to create migration without error"),
        "expected to create migration"
    );
    mem_migrations
}

/// Populate the database schema.
pub async fn setup_db_async(conn: &mut ConnectionAsync) {
    let mem_migrations = create_current_migrations(conn.backend());
    log::info!("created current migration");
    mem_migrations.migrate_async(conn).await.unwrap();
}

/// Populate the database schema.
pub fn setup_db(conn: &mut Connection) {
    let mem_migrations = create_current_migrations(conn.backend());
    log::info!("created current migration");
    mem_migrations.migrate(conn).unwrap();
}

/// Create a sqlite [`Connection`].
#[cfg(feature = "sqlite")]
pub fn sqlite_connection() -> Connection {
    let backend = get_backend(sqlite::BACKEND_NAME).unwrap();
    backend.connect(":memory:").unwrap()
}

/// Create a sqlite [`ConnectionSpec`].
#[cfg(feature = "sqlite")]
pub fn sqlite_connspec() -> ConnectionSpec {
    ConnectionSpec::new(sqlite::BACKEND_NAME, ":memory:")
}

/// Concrete [SetupData] for SQLite.
#[cfg(feature = "sqlite")]
pub struct SQLiteSetupData {}

#[cfg(feature = "sqlite")]
impl SetupData for SQLiteSetupData {
    fn connection_string(&self) -> &str {
        ":memory:"
    }
}

/// Setup the test sqlite database.
#[cfg(feature = "sqlite")]
pub async fn sqlite_setup() -> SQLiteSetupData {
    SQLiteSetupData {}
}
/// Tear down the test sqlite database.
#[cfg(feature = "sqlite")]
pub fn sqlite_teardown(_: SQLiteSetupData) {}

fn common_setup() {
    env_logger::try_init().ok();
}

/// Run a test function with a wrapper to set up and tear down the connection.
pub async fn run_test_async<T, Fut, Fut2>(
    backend_name: &str,
    setup: impl FnOnce() -> Fut,
    teardown: impl FnOnce(T),
    migrate: bool,
    test: impl FnOnce(ConnectionAsync) -> Fut2,
) where
    T: SetupData,
    Fut: Future<Output = T>,
    Fut2: Future<Output = ()>,
{
    env_logger::try_init().ok();
    let backend = get_backend(backend_name).expect("Could not find backend");
    let setup_data = setup().await;
    let connstr = setup_data.connection_string();
    log::info!("connecting to {}..", connstr);
    let mut conn = backend
        .connect_async(connstr)
        .await
        .expect("Could not connect backend");
    if migrate {
        setup_db_async(&mut conn).await;
    }
    log::info!("running test on {}...", connstr);
    test(conn).await;
    teardown(setup_data);
}

/// Wrap `$fname` in a `#[test]` with a `Connection` to `$connstr`.
#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $migrate:expr) => {
        paste::item! {
            #[tokio::test]
            pub async fn [<$fname _ $backend>]() {
                use butane_test_helper::*;
                match stringify!($backend) {
                    #[cfg(feature = "pg")]
                    "pg" => PgTestInstance::run_test_async($fname, $migrate).await,
                    #[cfg(feature = "sqlite")]
                    "sqlite" => SQLiteTestInstance::run_test_async($fname, $migrate).await,
                    _ => panic!("Unknown backend $backend")
                };
            }
        }
    };
}

/// Wrap `$fname` in a `#[test]` with a postgres `Connection` to `$connstr`.
#[macro_export]
macro_rules! maketest_pg {
    ($fname:ident, $migrate:expr) => {
        maketest!($fname, pg, $migrate);
    };
}
