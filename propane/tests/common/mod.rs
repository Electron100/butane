use propane::db::ConnectionSpec;
use propane::migrations::Migration;

pub mod blog;

pub fn setup_db(spec: &ConnectionSpec) {
    let mut root = std::env::current_dir().unwrap();
    root.push("propane/migrations");
    dbg!(&root);
    let migrations = propane::migrations::from_root(root);
    let current = migrations.get_current();
    let backend = propane::db::get_backend(&spec.backend_name)
        .expect(&format!("couldn't get db backend '{}'", &spec.backend_name));
    // todo using different migration names to avoid race is hacky, should find way to use different
    // migration directory for each test.
    let initial: Migration = migrations
        .create_migration_sql(backend, &format!("init_{}", spec.conn_str), None, &current)
        .expect("expected to create migration without error")
        .expect("expected non-None migration");
    let sql = initial.get_up_sql(&spec.backend_name).unwrap();
    let conn = propane::db::connect(spec).unwrap();
    conn.execute(&sql).unwrap();
}

pub fn reset_db(connstr: String) {
    std::fs::remove_file(connstr).ok();
}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                dbg!($connstr);
                crate::common::reset_db($connstr);
                let spec = ConnectionSpec::new(stringify!($backend), $connstr);
                crate::common::setup_db(&spec);
                let conn = propane::db::connect(&spec).unwrap();
                $fname(conn);
                //reset_db($connstr);
            }
        }
    };
}

#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, format!("test_{}.db", stringify!($fname)));
    };
}
