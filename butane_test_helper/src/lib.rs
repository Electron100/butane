use butane_core::db::{connect, get_backend, pg, sqlite, Backend, Connection, ConnectionSpec};
use butane_core::migrations::{self, MemMigrations, Migration, Migrations, MigrationsMut};
use once_cell::sync::Lazy;
use std::io::{BufRead, BufReader, Read, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use std::sync::Mutex;
use uuid::Uuid;

pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = get_backend(pg::BACKEND_NAME).unwrap();
    let data = pg_setup();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

pub fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup();
    (
        ConnectionSpec::new(pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

/// Server state for a test PostgreSQL server
#[derive(Debug)]
pub struct PgServerState {
    /// Temporary directory containing the test server
    pub dir: PathBuf,
    /// Directory for the socket
    pub sockdir: PathBuf,
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
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

/// Connection spec for a test server
#[derive(Clone, Debug)]
pub struct PgSetupData {
    /// Connection string
    pub connstr: String,
}

pub fn create_tmp_server() -> PgServerState {
    eprintln!("create tmp server");
    // create a temporary directory
    let dir = std::env::current_dir()
        .unwrap()
        .join("tmp_pg")
        .join(Uuid::new_v4().to_string());
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

    let sockdir = dir.join("socket");
    std::fs::create_dir(&sockdir).unwrap();

    // Run postgres to actually create the server
    let mut proc = Command::new("postgres")
        .arg("-c")
        .arg("logging_collector=false")
        .arg("-D")
        .arg(&dir)
        .arg("-k")
        .arg(&sockdir)
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
        if buf.contains("ready to accept connections") {
            break;
        }
        if proc.try_wait().unwrap().is_some() {
            buf.clear();
            stderr.read_to_string(&mut buf).unwrap();
            eprint!("{buf}");
            panic!("postgres process died");
        }
    }
    eprintln!("created tmp server!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
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

pub fn pg_setup() -> PgSetupData {
    eprintln!("pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR
    let connstr = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(connstr) => connstr,
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();
            let host = server.sockdir.to_str().unwrap();
            format!("host={host} user=postgres")
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    eprintln!("new db is `{}`", &new_dbname);

    let mut conn = connect(&ConnectionSpec::new("pg", &connstr)).unwrap();
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .unwrap();

    let connstr = format!("{connstr} dbname={new_dbname}");
    PgSetupData { connstr }
}
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

pub fn pg_connstr(data: &PgSetupData) -> String {
    data.connstr.clone()
}

pub fn setup_db(backend: Box<dyn Backend>, conn: &mut Connection, migrate: bool) {
    let mut root = std::env::current_dir().unwrap();
    root.push(".butane/migrations");
    let mut disk_migrations = migrations::from_root(&root);
    let disk_current = disk_migrations.current();
    eprintln!("{:?}", disk_current);
    if !migrate {
        return;
    }
    // Create an in-memory Migrations and write only to that. This
    // allows concurrent tests to avoid stomping on each other and is
    // also faster than real disk writes.
    let mut mem_migrations = MemMigrations::new();
    let mem_current = mem_migrations.current();

    migrations::copy_migration(disk_current, mem_current).unwrap();

    assert!(
        mem_migrations
            .create_migration(&backend, "init", None)
            .expect("expected to create migration without error"),
        "expected to create migration"
    );
    println!("created current migration");
    let to_apply = mem_migrations.unapplied_migrations(conn).unwrap();
    for m in to_apply {
        println!("Applying migration {}", m.name());
        m.apply(conn).unwrap();
    }
}

pub fn sqlite_connection() -> Connection {
    let backend = get_backend(sqlite::BACKEND_NAME).unwrap();
    backend.connect(":memory:").unwrap()
}

pub fn sqlite_connspec() -> ConnectionSpec {
    ConnectionSpec::new(sqlite::BACKEND_NAME, ":memory:")
}

pub fn sqlite_setup() {}
pub fn sqlite_teardown(_: ()) {}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr, $dataname:ident, $migrate:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                env_logger::try_init().ok();
                let backend = butane_core::db::get_backend(&stringify!($backend)).expect("Could not find backend");
                let $dataname = butane_test_helper::[<$backend _setup>]();
                eprintln!("connecting to {}", &$connstr);
                let mut conn = backend.connect(&$connstr).expect("Could not connect backend");
                butane_test_helper::setup_db(backend, &mut conn, $migrate);
                $fname(conn);
                butane_test_helper::[<$backend _teardown>]($dataname);
            }
        }
    };
}

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
