use paste;
use propane::db::Connection;
use propane::find;
use propane::model;
use propane::prelude::*;
use propane::query;
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

fn basic_find(conn: Connection) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).unwrap();

    // find
    let found = find!(Foo, bar == 43, &conn).unwrap();
    assert_eq!(found, foo2);
}
testall!(basic_find);

fn basic_query(conn: Connection) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).unwrap();

    // query finds 1
    let mut found = query!(Foo, bar == 42).load(&conn).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found.pop().unwrap(), foo1);

    // query finds both
    let found = query!(Foo, bar < 44).load(&conn).unwrap();
    assert_eq!(found.len(), 2);
}
testall!(basic_query);

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
