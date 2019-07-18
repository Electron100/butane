use paste;
use propane::db::{Connection, ConnectionSpec};
use propane::migrations::Migration;
use propane::model;
use propane::prelude::*;

#[model]
#[derive(PartialEq, Eq, Debug)]
struct Foo {
    id: i64,
    bar: u32,
    baz: String,
}
impl Foo {
    fn new() -> Self {
        Foo {
            id: 0,
            bar: 0,
            baz: String::new(),
        }
    }
}

fn setup_db(spec: &ConnectionSpec) {
    let mut root = std::env::current_dir().unwrap();
    root.push("propane/migrations");
    let migrations = propane::migrations::from_root(root);
    let current = migrations.get_current();
    let backend = propane::db::get_backend(&spec.backend_name)
        .expect(&format!("couldn't get db backend '{}'", &spec.backend_name));
    let initial: Migration = migrations
        .create_migration_sql(backend, "init", None, &current)
        .expect("expected to create migration without error")
        .expect("expected non-None migration");
    let sql = initial.get_up_sql(&spec.backend_name).unwrap();
    let conn = propane::db::connect(spec).unwrap();
    conn.execute(&sql).unwrap();
}

fn reset_db() {
    std::fs::remove_file("test.db").ok();
}

macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                reset_db();
                let spec = ConnectionSpec::new(stringify!($backend), $connstr);
                setup_db(&spec);
                let conn = propane::db::connect(&spec).unwrap();
                $fname(conn)
            }
        }
    };
}

macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, "test.db");
    };
}

fn basic_crud(conn: Connection) {
    //create
    let mut foo = Foo::new();
    foo.id = 1;
    foo.bar = 42;
    foo.baz = "hello world".to_string();
    assert!(foo.save(&conn).is_ok());

    // read
    let mut foo2 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    assert!(foo2.save(&conn).is_ok());
    let foo3 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo2, foo3);

    // delete
    assert!(foo3.delete(&conn).is_ok());
    if let Some(propane::Error::NoSuchObject) = Foo::get(&conn, 1).err() {
    } else {
        panic!("Expected NoSuchObject");
    }
}
testall!(basic_crud);
