//! Test helpers to set up database connections.
//!
//! Macros depend on [`butane_core`], `env_logger` and [`log`].
//! Public members of this crate should not be considered supported.
//! This crate is not published to crates.io.
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

use std::future::Future;
#[cfg(feature = "pg")]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(feature = "pg")]
use std::ops::Deref;
#[cfg(feature = "pg")]
use std::process::{ChildStderr, Command, Stdio};
#[cfg(feature = "pg")]
use std::sync::{LazyLock, Mutex};

#[cfg(feature = "pg")]
use block_id::{Alphabet, BlockId};
#[cfg(feature = "pg")]
use butane_core::db::pg::PgBackend;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite::SQLiteBackend;
#[cfg(feature = "turso")]
use butane_core::db::turso;
#[cfg(feature = "turso")]
use butane_core::db::turso::TursoBackend;
#[cfg(feature = "pg")]
use butane_core::db::{connect, connect_async};
use butane_core::db::{get_backend, Backend, ConnectionSpec};
use butane_core::migrations::{self, MemMigrations, Migration, Migrations, MigrationsMut};
#[cfg(feature = "pg")]
use uuid::Uuid;

extern crate alloc;

// Re-export as they are used by the macros.
pub use butane_core::db::{BackendConnection, BackendConnectionAsync, Connection, ConnectionAsync};
pub use maybe_async_cfg;

#[cfg(feature = "pg")]
pub mod pg;

// Re-export types from pg module
#[cfg(feature = "pg")]
pub use crate::pg::{pg_tmp_server_create_ephemeralpg, PgTemporaryServerError};

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

/// Instance of a Turso test.
#[cfg(feature = "turso")]
#[derive(Default)]
pub struct TursoTestInstance {}

#[cfg(feature = "turso")]
impl BackendTestInstance for TursoTestInstance {
    fn run_test_sync(_test: impl FnOnce(Connection), _migrate: bool) {
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
    let backend = get_backend(butane_core::db::pg::BACKEND_NAME).unwrap();
    let data = pg_setup_sync();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

/// Create a PostgreSQL [`ConnectionSpec`].
#[cfg(feature = "pg")]
pub async fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup().await;
    (
        ConnectionSpec::new(butane_core::db::pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

/// Options for creating a PostgreSQL server.
#[cfg(feature = "pg")]
#[derive(Clone, Debug, Default)]
pub struct PgServerOptions {
    /// The port to listen on. If None, only allow connections via unix sockets.
    pub port: Option<u16>,
    /// The user to connect as. If None, use the default user.
    pub user: Option<String>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    /// Use abstract namespace for the socket.
    ///
    /// Postgres only supports this on Linux and Windows.
    /// However rust-postgres does not yet support it.
    /// <https://github.com/sfackler/rust-postgres/issues/1240>
    pub abstract_namespace: bool,
    /// Callback to run at exit.
    pub atexit_callback: Option<extern "C" fn()>,
    /// Wait time in seconds before automatic cleanup (for ephemeralpg).
    /// If None, uses pg_tmp's default (60 seconds).
    pub ephemeralpg_wait_seconds: Option<u32>,
}

/// Server state for a test PostgreSQL server.
#[cfg(feature = "pg")]
#[derive(Debug)]
pub struct PgServerState {
    /// Temporary directory containing the test server (not used for ephemeralpg).
    pub dir: std::path::PathBuf,
    /// Directory for the socket (not used for ephemeralpg).
    pub sockdir: tempfile::TempDir,
    /// Process of the test server.
    pub proc: std::process::Child,
    /// stderr from the test server.
    pub stderr: BufReader<ChildStderr>,
    /// Options used to create the server.
    pub options: PgServerOptions,
    /// Connection URI from pg_tmp (only set when using ephemeralpg).
    pub ephemeralpg_uri: Option<String>,
}

#[cfg(feature = "pg")]
impl Drop for PgServerState {
    fn drop(&mut self) {
        // Avoid using Child.kill on Unix, as it uses SIGKILL, which postgresql recommends against,
        // and is known to cause shared memory leakage on macOS.
        // See Notes section of https://www.postgresql.org/docs/current/app-postgres.html
        #[cfg(windows)]
        self.proc.kill().ok();
        #[cfg(not(windows))]
        unsafe {
            libc::kill(self.proc.id() as i32, libc::SIGTERM);
        }

        // Wait for the process to exit
        let mut buf = String::new();
        self.stderr.read_to_string(&mut buf).unwrap();
        if !buf.is_empty() {
            log::warn!("pg shutdown error: {buf}");
        }

        // Clean up shared memory segments (macOS-specific issue)
        #[cfg(target_os = "macos")]
        if !self.dir.as_os_str().is_empty() {
            pg::cleanup_macos_postgres_shared_memory(&self.dir);
        }

        // Only delete directory for custom postgres, not for ephemeralpg
        if self.ephemeralpg_uri.is_none() && !self.dir.as_os_str().is_empty() {
            log::info!("Deleting {}", self.dir.display());
            std::fs::remove_dir_all(&self.dir).unwrap();
        }
    }
}

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

/// Create and start a temporary PostgreSQL server instance.
///
/// This function automatically detects and prefers `pg_tmp` (ephemeralpg) if available,
/// otherwise falls back to using `initdb` and `postgres` directly.
///
/// Fails on Windows CI due to:
/// > The server must be started under an unprivileged user ID to prevent
/// > possible system security compromises. ...
#[cfg(feature = "pg")]
pub fn pg_tmp_server_create(
    options: PgServerOptions,
) -> Result<PgServerState, PgTemporaryServerError> {
    // Try pg_tmp first if available
    if pg::is_pg_tmp_available() {
        log::debug!("pg_tmp detected, using ephemeralpg");
        pg_tmp_server_create_ephemeralpg(options)
    } else if pg::is_initdb_available() {
        log::debug!("pg_tmp not found, falling back to initdb");
        pg_tmp_server_create_using_initdb(options)
    } else {
        Err(PgTemporaryServerError::EphemeralPg(
            "Neither pg_tmp nor initdb found in PATH. Please install ephemeralpg or PostgreSQL."
                .to_string(),
        ))
    }
}

/// Create and start a temporary PostgreSQL server instance using initdb.
#[cfg(feature = "pg")]
pub fn pg_tmp_server_create_using_initdb(
    options: PgServerOptions,
) -> Result<PgServerState, PgTemporaryServerError> {
    // Otherwise use the custom postgres implementation
    let seed: u128 = rand::random::<u64>() as u128;
    let instance_id = BlockId::new(Alphabet::alphanumeric(), seed, 8)
        .encode_string(0)
        .unwrap();
    // create a temporary directory
    let dir = std::env::current_dir()
        .unwrap()
        .join("tmp_pg")
        .join(instance_id);
    std::fs::create_dir_all(&dir).unwrap();

    let user = options
        .user
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    // Run initdb to create a postgres cluster in our temporary director
    let result = Command::new("initdb")
        .arg("-D")
        .arg(&dir)
        .arg("-U")
        .arg(user)
        .output();
    if let Err(e) = result {
        eprintln!("failed to run initdb; PostgreSQL may not be installed");
        return Err(e.into());
    }

    let output = result.unwrap();

    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("PostgreSQL initdb failed")
    }

    let sockdir = tempfile::TempDir::new().unwrap();

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let socket_directory_arg = if options.abstract_namespace {
        // Use abstract namespace for the socket
        format!("@{}", sockdir.path().display())
    } else {
        // Use a normal socket
        sockdir.path().display().to_string()
    };
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let socket_directory_arg = sockdir.path().display().to_string();

    // Run postgres to actually create the server
    // See https://www.postgresql.org/docs/current/app-postgres.html for CLI args.
    // PGOPTIONS can be used to set args.
    // PGDATA can be used instead of -D
    let mut command = Command::new("postgres");
    command
        .arg("-c")
        .arg("logging_collector=false")
        .arg("-D")
        .arg(&dir)
        .arg("-k")
        .arg(socket_directory_arg)
        .stderr(Stdio::piped());

    if let Some(port) = options.port {
        command
            .arg("-i")
            .arg("-h")
            .arg("localhost")
            .arg("-p")
            .arg(port.to_string());
    } else {
        // Set host='' to prevent postgres from trying to use TCP/IP
        command.arg("-h").arg("");
    }
    let result = command.spawn();
    if let Err(e) = result {
        eprintln!("failed to run postgres");
        return Err(e.into());
    }

    let mut proc = result.unwrap();

    let mut buf = String::new();
    let mut stderr = BufReader::new(proc.stderr.take().unwrap());
    loop {
        buf.clear();
        stderr.read_line(&mut buf).unwrap();
        log::trace!("{buf}");
        if buf.contains("ready to accept connections") {
            break;
        }
        if proc.try_wait().unwrap().is_some() {
            buf.clear();
            stderr.read_to_string(&mut buf).unwrap();
            log::error!("{buf}");
            panic!("postgres process died");
        }
    }
    log::info!("created tmp pg server {}", sockdir.path().display());
    if let Some(cb) = options.atexit_callback {
        // Register the callback to be called when the process exits
        // This is not safe, but this is in a test context and the process will exit.
        // Use `atexit` to ensure that the callback is called even if
        // the process exits unexpectedly.
        log::info!("registering atexit callback");
        unsafe {
            libc::atexit(cb);
        }
    }

    Ok(PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
        options: options.clone(),
        ephemeralpg_uri: None,
    })
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
            if let Some(uri) = &server.ephemeralpg_uri {
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
            if let Some(uri) = &server.ephemeralpg_uri {
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

/// Get a synchronous connection by backend name.
///
/// Panics for turso backend (async only).
pub fn get_sync_connection(backend_name: &str) -> Connection {
    match backend_name {
        #[cfg(feature = "sqlite")]
        "sqlite" => sqlite_connection(),
        #[cfg(feature = "pg")]
        "pg" => {
            let (conn, _data) = pg_connection();
            conn
        }
        #[cfg(feature = "turso")]
        "turso" => panic!("turso not supported for synchronous connections (async only)"),
        _ => panic!("Unsupported backend: {}", backend_name),
    }
}

/// Get an asynchronous connection by backend name.
pub async fn get_async_connection(backend_name: &str) -> ConnectionAsync {
    match backend_name {
        #[cfg(feature = "sqlite")]
        "sqlite" => {
            let backend = get_backend(sqlite::BACKEND_NAME).unwrap();
            backend.connect_async(":memory:").await.unwrap()
        }
        #[cfg(feature = "pg")]
        "pg" => {
            let backend = get_backend(butane_core::db::pg::BACKEND_NAME).unwrap();
            let data = pg_setup().await;
            backend
                .connect_async(data.connection_string())
                .await
                .unwrap()
        }
        #[cfg(feature = "turso")]
        "turso" => {
            let backend = get_backend(turso::BACKEND_NAME).unwrap();
            backend.connect_async(":memory:").await.unwrap()
        }
        _ => panic!("Unsupported backend: {}", backend_name),
    }
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

/// Create a turso connection.
#[cfg(feature = "turso")]
pub fn turso_connection() -> Connection {
    let backend = get_backend(turso::BACKEND_NAME).unwrap();
    backend.connect(":memory:").unwrap()
}

/// Create a turso [`ConnectionSpec`].
#[cfg(feature = "turso")]
pub fn turso_connspec() -> ConnectionSpec {
    ConnectionSpec::new(turso::BACKEND_NAME, ":memory:")
}

/// Concrete [SetupData] for Turso.
#[cfg(feature = "turso")]
pub struct TursoSetupData {}

#[cfg(feature = "turso")]
impl SetupData for TursoSetupData {
    fn connection_string(&self) -> &str {
        ":memory:"
    }
}

/// Setup the test turso database.
#[cfg(feature = "turso")]
pub async fn turso_setup() -> TursoSetupData {
    TursoSetupData {}
}

/// Tear down the test turso database.
#[cfg(feature = "turso")]
pub fn turso_teardown(_: TursoSetupData) {}

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
                    #[cfg(feature = "turso")]
                    "turso" => TursoTestInstance::run_test_async($fname, $migrate).await,
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

/// Wrap `$fname` in a `#[test]` with a turso `Connection`.
#[macro_export]
macro_rules! maketest_turso {
    ($fname:ident, $migrate:expr) => {
        maketest!($fname, turso, $migrate);
    };
}
