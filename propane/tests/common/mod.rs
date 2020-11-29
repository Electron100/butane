#![allow(dead_code)] //this module is used by multiple tests, not all use all parts
use propane::db::{Backend, Connection};
use propane::migrations::{MemMigrations, Migration, Migrations, MigrationsMut};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use uuid_for_test::Uuid;

pub mod blog;

pub fn setup_db(backend: Box<dyn Backend>, conn: &mut Connection) {
    let mut root = std::env::current_dir().unwrap();
    root.push("propane/migrations");
    let mut disk_migrations = propane::migrations::from_root(&root);
    let disk_current = disk_migrations.current();
    // Create an in-memory Migrations and write only to that. This
    // allows concurrent tetss to avoid stomping on eachother and is
    // also faster than real disk writes.
    let mut mem_migrations = MemMigrations::new();
    let mem_current = mem_migrations.current();

    propane::migrations::copy_migration(disk_current, mem_current).unwrap();

    assert!(
        mem_migrations
            .create_migration(&backend, &format!("init"), None)
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
    let backend = propane::db::get_backend("sqlite").unwrap();
    backend.connect(":memory:").unwrap()
}

pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = propane::db::get_backend("pg").unwrap();
    let data = pg_setup();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

pub fn sqlite_setup() {}
pub fn sqlite_teardown(_: ()) {}
pub struct PgSetupData {
    pub dir: PathBuf,
    pub sockdir: PathBuf,
    pub proc: std::process::Child,
    // stderr from the child process
    pub stderr: BufReader<ChildStderr>,
}
impl Drop for PgSetupData {
    fn drop(&mut self) {
        self.proc.kill().ok();
        let mut buf = String::new();
        self.stderr.read_to_string(&mut buf).unwrap();
        eprintln!("postgres stderr is {}", buf);
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

pub fn pg_setup() -> PgSetupData {
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
            eprint!("{}", buf);
            panic!("postgres process died");
        }
    }
    PgSetupData {
        dir,
        sockdir,
        proc,
        stderr,
    }
}
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

pub fn pg_connstr(data: &PgSetupData) -> String {
    format!(
        "host={} dbname=postgres user=postgres",
        data.sockdir.to_str().unwrap()
    )
}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr, $dataname:ident) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                let backend = propane::db::get_backend(&stringify!($backend)).expect("Could not find backend");
								let $dataname = crate::common::[<$backend _setup>]();
								eprintln!("connecting to {}", &$connstr);
                let mut conn = backend.connect(&$connstr).expect("Could not connect backend");
                crate::common::setup_db(backend, &mut conn);
                $fname(conn);
								crate::common::[<$backend _teardown>]($dataname);
            }
        }
    };
}

#[macro_export]
macro_rules! maketest_pg {
    ($fname:ident) => {
        maketest!(
            $fname,
            pg,
            crate::common::pg_connstr(&setup_data),
            setup_data
        );
    };
}

#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, format!(":memory:"), setup_data);
        maketest_pg!($fname);
    };
}
