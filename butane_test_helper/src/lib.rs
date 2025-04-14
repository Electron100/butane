//! Test helpers to set up database connections.
//! Macros depend on [`butane_core`], `env_logger` and [`log`].
#![deny(missing_docs)]

extern crate alloc;

use std::future::Future;
#[cfg(feature = "pg")]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(feature = "pg")]
use std::ops::Deref;
#[cfg(feature = "pg")]
use std::path::PathBuf;
#[cfg(feature = "pg")]
use std::process::{ChildStderr, Command, Stdio};
#[cfg(feature = "pg")]
use std::sync::Mutex;

#[cfg(feature = "pg")]
use block_id::{Alphabet, BlockId};
#[cfg(feature = "pg")]
use butane_core::db::pg::PgBackend;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite;
#[cfg(feature = "sqlite")]
use butane_core::db::sqlite::SQLiteBackend;
#[cfg(feature = "pg")]
use butane_core::db::{connect, connect_async, pg};
use butane_core::db::{get_backend, Backend, ConnectionSpec};

use butane_core::migrations::{self, MemMigrations, Migration, Migrations, MigrationsMut};
#[cfg(feature = "pg")]
use once_cell::sync::Lazy;
#[cfg(feature = "pg")]
use uuid::Uuid;

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
        let connstr = setup_data.connstr;
        log::info!("connecting to {}..", connstr);
        let mut conn = backend
            .connect(&connstr)
            .expect("Could not connect backend");
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
        let connstr = setup_data.connstr();
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
    fn connstr(&self) -> &str;
}

/// Create a PostgreSQL [`Connection`].
#[cfg(feature = "pg")]
pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = get_backend(pg::BACKEND_NAME).unwrap();
    let data = pg_setup_sync();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

/// Create a PostgreSQL [`ConnectionSpec`].
#[cfg(feature = "pg")]
pub async fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup().await;
    (
        ConnectionSpec::new(pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

/// Server state for a test PostgreSQL server.
#[cfg(feature = "pg")]
#[derive(Debug)]
pub struct PgServerState {
    /// Temporary directory containing the test server
    pub dir: PathBuf,
    /// Directory for the socket
    pub sockdir: tempfile::TempDir,
    /// Process of the test server
    pub proc: std::process::Child,
    /// stderr from the test server
    pub stderr: BufReader<ChildStderr>,
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

        log::info!("Deleting {}", self.dir.display());
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

/// Connection spec for a test server.
#[cfg(feature = "pg")]
#[derive(Clone, Debug)]
pub struct PgSetupData {
    /// Connection string
    pub connstr: String,
}
#[cfg(feature = "pg")]
impl SetupData for PgSetupData {
    fn connstr(&self) -> &str {
        &self.connstr
    }
}

/// Create and start a temporary PostgreSQL server instance.
#[cfg(feature = "pg")]
pub fn create_tmp_server() -> PgServerState {
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

    // Run initdb to create a postgres cluster in our temporary director
    let output = Command::new("initdb")
        .arg("-D")
        .arg(&dir)
        .arg("-U")
        .arg("postgres")
        .output()
        .expect("failed to run initdb; PostgreSQL may not be installed.");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("PostgreSQL initdb failed")
    }

    let sockdir = tempfile::TempDir::new().unwrap();

    // Run postgres to actually create the server
    // See https://www.postgresql.org/docs/current/app-postgres.html for CLI args.
    // PGOPTIONS can be used to set args.
    // PGDATA can be used instead of -D
    let mut proc = Command::new("postgres")
        .arg("-c")
        .arg("logging_collector=false")
        .arg("-D")
        .arg(&dir)
        .arg("-k")
        .arg(sockdir.path())
        .arg("-h")
        .arg("")
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run postgres");
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
    log::info!("created tmp pg server.");
    unsafe {
        // Try to delete all the pg files when the process exits
        libc::atexit(proc_teardown);
    }
    PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
    }
}

#[cfg(feature = "pg")]
extern "C" fn proc_teardown() {
    drop(TMP_SERVER.deref().lock().unwrap().take());
}

#[cfg(feature = "pg")]
static TMP_SERVER: Lazy<Mutex<Option<PgServerState>>> =
    Lazy::new(|| Mutex::new(Some(create_tmp_server())));

/// Create a running empty PostgreSQL database named `butane_test_<uuid>`.
#[cfg(feature = "pg")]
pub fn pg_setup_sync() -> PgSetupData {
    log::trace!("starting pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR
    let connstr = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(connstr) => connstr,
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();
            let host = server.sockdir.path().to_str().unwrap();
            format!("host={host} user=postgres")
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    log::info!("new db is `{}`", &new_dbname);

    let conn = connect(&ConnectionSpec::new("pg", &connstr)).unwrap();
    log::debug!("closed is {}", BackendConnection::is_closed(&conn));
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .unwrap();

    let connstr = format!("{connstr} dbname={new_dbname}");
    PgSetupData { connstr }
}

/// Create a running empty PostgreSQL database named `butane_test_<uuid>`.
#[cfg(feature = "pg")]
pub async fn pg_setup() -> PgSetupData {
    log::trace!("starting pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR
    let connstr = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(connstr) => connstr,
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();
            let host = server.sockdir.path().to_str().unwrap();
            format!("host={host} user=postgres")
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    log::info!("new db is `{}`", &new_dbname);

    let conn = connect_async(&ConnectionSpec::new("pg", &connstr))
        .await
        .unwrap();
    log::debug!(
        "[async]closed is {}",
        BackendConnectionAsync::is_closed(&conn)
    );
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .await
        .unwrap();

    let connstr = format!("{connstr} dbname={new_dbname}");
    PgSetupData { connstr }
}

/// Tear down PostgreSQL database created by [`pg_setup`].
#[cfg(feature = "pg")]
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

/// Obtain the connection string for the PostgreSQL database.
#[cfg(feature = "pg")]
pub fn pg_connstr(data: &PgSetupData) -> String {
    data.connstr.clone()
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
    fn connstr(&self) -> &str {
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
    let connstr = setup_data.connstr();
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
