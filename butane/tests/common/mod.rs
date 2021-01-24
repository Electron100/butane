#![allow(dead_code)] //this module is used by multiple tests, not all use all parts
use butane::db::{Backend, Connection, ConnectionSpec};
use butane::migrations::{MemMigrations, Migration, Migrations, MigrationsMut};

pub mod blog;
pub mod pg;
pub use pg::*;

pub fn setup_db(backend: Box<dyn Backend>, conn: &mut Connection) {
    let mut root = std::env::current_dir().unwrap();
    root.push(".butane/migrations");
    let mut disk_migrations = butane::migrations::from_root(&root);
    let disk_current = disk_migrations.current();
    // Create an in-memory Migrations and write only to that. This
    // allows concurrent tetss to avoid stomping on eachother and is
    // also faster than real disk writes.
    let mut mem_migrations = MemMigrations::new();
    let mem_current = mem_migrations.current();

    butane::migrations::copy_migration(disk_current, mem_current).unwrap();

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
    let backend = butane::db::get_backend("sqlite").unwrap();
    backend.connect(":memory:").unwrap()
}

pub fn sqlite_connspec() -> ConnectionSpec {
    ConnectionSpec::new(butane::db::sqlite::BACKEND_NAME, ":memory:")
}

pub fn sqlite_setup() {}
pub fn sqlite_teardown(_: ()) {}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr, $dataname:ident) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                let backend = butane::db::get_backend(&stringify!($backend)).expect("Could not find backend");
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
            &crate::common::pg_connstr(&setup_data),
            setup_data
        );
    };
}

#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "sqlite")] {
                maketest!($fname, sqlite, &format!(":memory:"), setup_data);
            }
        }
        cfg_if::cfg_if! {
            if #[cfg(feature = "pg")] {
                maketest_pg!($fname);
            }
        }
    };
}
