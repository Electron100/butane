use propane::db::{Backend, Connection};
use propane::migrations::Migration;

pub mod blog;

pub fn setup_db(backend: Box<Backend>, conn: &Connection, name: &str) {
    let mut root = std::env::current_dir().unwrap();
    root.push("propane/migrations");
    let migrations = propane::migrations::from_root(root);
    let current = migrations.get_current();
    // todo using different migration names to avoid race is hacky, should find way to use different
    // migration directory for each test.
    let initial: Migration = migrations
        .create_migration_sql(&backend, &format!("init_{}", name), None, &current)
        .expect("expected to create migration without error")
        .expect("expected non-None migration");
    let sql = initial.get_up_sql(backend.get_name()).unwrap();
    conn.execute(&sql).unwrap();
}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                let backend = propane::db::get_backend(&stringify!($backend)).unwrap();
                let conn = backend.connect(&$connstr).unwrap();
                crate::common::setup_db(backend, &conn, &stringify!($fname));
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
