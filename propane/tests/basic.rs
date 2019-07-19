use paste;
use propane::db::{Connection, ConnectionSpec};
use propane::model;
use propane::prelude::*;
use propane::ForeignKey;

mod common;

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
