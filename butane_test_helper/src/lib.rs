//! Test helpers to set up database connections.
//! Macros depend on [`butane_core`], `env_logger` and [`log`].
#![deny(missing_docs)]

use std::io::{BufRead, BufReader, Read, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use std::sync::Mutex;

use block_id::{Alphabet, BlockId};
use butane_core::db::{
    connect, get_backend, pg, sqlite, Backend, BackendConnection, Connection, ConnectionSpec,
};
use butane_core::migrations::{self, MemMigrations, Migration, Migrations, MigrationsMut};
use once_cell::sync::Lazy;
use uuid::Uuid;

/// Create a postgres [`Connection`].
pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = get_backend(pg::BACKEND_NAME).unwrap();
    let data = pg_setup();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

/// Create a postgres [`ConnectionSpec`].
pub fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup();
    (
        ConnectionSpec::new(pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

/// Server state for a test PostgreSQL server.
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
impl Drop for PgServerState {
    fn drop(&mut self) {
        self.proc.kill().ok();
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
#[derive(Clone, Debug)]
pub struct PgSetupData {
    /// Connection string
    pub connstr: String,
}

/// Create and start a temporary postgres server instance.
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
        .expect("failed to run initdb");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("postgres initdb failed")
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

extern "C" fn proc_teardown() {
    drop(TMP_SERVER.deref().lock().unwrap().take());
}

static TMP_SERVER: Lazy<Mutex<Option<PgServerState>>> =
    Lazy::new(|| Mutex::new(Some(create_tmp_server())));

/// Create a running empty postgres database named `butane_test_<uuid>`.
pub fn pg_setup() -> PgSetupData {
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

    let mut conn = connect(&ConnectionSpec::new("pg", &connstr)).unwrap();
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .unwrap();

    let connstr = format!("{connstr} dbname={new_dbname}");
    PgSetupData { connstr }
}

/// Tear down postgres database created by [`pg_setup`].
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

/// Obtain the connection string for the postgres database.
pub fn pg_connstr(data: &PgSetupData) -> String {
    data.connstr.clone()
}

/// Create a [`MemMigrations`]` for the "current" migration.
pub fn create_current_migrations(connection: &Connection) -> MemMigrations {
    let backend = connection.backend();

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
pub fn setup_db(conn: &mut Connection) {
    let mem_migrations = create_current_migrations(conn);
    log::info!("created current migration");
    mem_migrations.migrate(conn).unwrap();
}

/// Create a sqlite [`Connection`].
pub fn sqlite_connection() -> Connection {
    let backend = get_backend(sqlite::BACKEND_NAME).unwrap();
    backend.connect(":memory:").unwrap()
}

/// Create a sqlite [`ConnectionSpec`].
pub fn sqlite_connspec() -> ConnectionSpec {
    ConnectionSpec::new(sqlite::BACKEND_NAME, ":memory:")
}

/// Setup the test sqlite database.
pub fn sqlite_setup() {}
/// Tear down the test sqlite database.
pub fn sqlite_teardown(_: ()) {}

/// Wrap `$fname` in a `#[test]` with a `Connection` to `$connstr`.
#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr, $dataname:ident, $migrate:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                env_logger::try_init().ok();
                let backend = butane_core::db::get_backend(&stringify!($backend)).expect("Could not find backend");
                let $dataname = butane_test_helper::[<$backend _setup>]();
                log::info!("connecting to {}..", &$connstr);
                let mut conn = backend.connect(&$connstr).expect("Could not connect backend");
                if $migrate {
                    butane_test_helper::setup_db(&mut conn);
                }
                log::info!("running test on {}..", &$connstr);
                $fname(conn);
                butane_test_helper::[<$backend _teardown>]($dataname);
            }
        }
    };
}

/// Wrap `$fname` in a `#[test]` with a postgres `Connection` to `$connstr`.
#[macro_export]
macro_rules! maketest_pg {
    ($fname:ident, $migrate:expr) => {
        maketest!(
            $fname,
            pg,
            &butane_test_helper::pg_connstr(&setup_data),
            setup_data,
            $migrate
        );
    };
}

/// Create a sqlite and postgres `#[test]` that each invoke `$fname` with a [`Connection`] containing the schema.
#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "sqlite")] {
                maketest!($fname, sqlite, &format!(":memory:"), setup_data, true);
            }
        }
        cfg_if::cfg_if! {
            if #[cfg(feature = "pg")] {
                maketest_pg!($fname, true);
            }
        }
    };
}

/// Create a sqlite and postgres `#[test]` that each invoke `$fname` with a [`Connection`] with no schema.
#[macro_export]
macro_rules! testall_no_migrate {
    ($fname:ident) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "sqlite")] {
                maketest!($fname, sqlite, &format!(":memory:"), setup_data, false);
            }
        }
        cfg_if::cfg_if! {
            if #[cfg(feature = "pg")] {
                maketest_pg!($fname, false);
            }
        }
    };
}
