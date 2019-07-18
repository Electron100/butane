use paste;
use propane::db::{Connection, ConnectionSpec};
use propane::model;
use propane::prelude::*;
use propane::{migrations::Migration, ForeignKey};

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct Foo {
    id: i64,
    bar: u32,
    baz: String,
}
impl Foo {
    fn new(id: i64) -> Self {
        Foo {
            id: id,
            bar: 0,
            baz: String::new(),
        }
    }
}

#[model]
#[derive(PartialEq, Eq, Debug)]
struct Bar {
    #[pk]
    name: String,
    foo: ForeignKey<Foo>,
}
impl Bar {
    fn new(name: &str, foo: Foo) -> Self {
        Bar {
            name: name.to_string(),
            foo: foo.into(),
        }
    }
}

fn setup_db(spec: &ConnectionSpec) {
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

fn reset_db(connstr: String) {
    std::fs::remove_file(connstr).ok();
}

macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                dbg!($connstr);
                reset_db($connstr);
                let spec = ConnectionSpec::new(stringify!($backend), $connstr);
                setup_db(&spec);
                let conn = propane::db::connect(&spec).unwrap();
                $fname(conn);
                //reset_db($connstr);
            }
        }
    };
}

macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, format!("test_{}.db", stringify!($fname)));
    };
}

fn basic_crud(conn: Connection) {
    //create
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.baz = "hello world".to_string();
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
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

fn string_pk(conn: Connection) {
    let mut foo = Foo::new(1);
    foo.save(&conn).unwrap();
    let mut bar = Bar::new("tarzan", foo);
    bar.save(&conn).unwrap();

    let bar2 = Bar::get(&conn, "tarzan".to_string()).unwrap();
    assert_eq!(bar, bar2);
}
testall!(string_pk);

fn foreign_key(conn: Connection) {
    let mut foo = Foo::new(1);
    foo.save(&conn).unwrap();
    let mut bar = Bar::new("tarzan", foo.clone());
    bar.save(&conn).unwrap();
    let bar2 = Bar::get(&conn, "tarzan".to_string()).unwrap();
    
    let foo2: &Foo = bar2.foo.load(&conn).unwrap();
    assert_eq!(&foo, foo2);

    let foo3: &Foo = bar2.foo.get().unwrap();
    assert_eq!(foo2, foo3);
}
testall!(foreign_key);