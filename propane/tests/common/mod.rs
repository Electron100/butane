use propane::db::{Backend, Connection};
use propane::migrations::{MemMigrations, Migration, Migrations, MigrationsMut};

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

    propane::migrations::copy_migration(disk_current, mem_current, None).unwrap();

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

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                let backend = propane::db::get_backend(&stringify!($backend)).unwrap();
                let mut conn = backend.connect(&$connstr).unwrap();
                crate::common::setup_db(backend, &mut conn);
                $fname(conn);
            }
        }
    };
}

#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, format!(":memory:"));
    };
}
