use butane::db::Connection;
use butane::{butane_type, find, model, query};
use butane::{colname, prelude::*};
use butane::{ForeignKey, ObjectState};
use chrono::{naive::NaiveDateTime, offset::Utc, DateTime};
use serde::Serialize;

use butane_test_helper::*;

#[butane_type]
pub type Whatsit = String;

// Note, Serialize derive exists solely to exercise the logic in butane_core::codegen::has_derive_serialize
#[model]
#[derive(PartialEq, Debug, Clone, Serialize)]
struct Foo {
    id: i64,
    bam: f64,
    #[unique]
    bar: u32,
    baz: Whatsit,
    blobbity: Vec<u8>,
}
impl Foo {
    fn new(id: i64) -> Self {
        Foo {
            id,
            bam: 0.0,
            bar: 0,
            baz: String::new(),
            blobbity: Vec::new(),
            state: ObjectState::default(),
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
            state: ObjectState::default(),
        }
    }
}

#[model]
struct Baz {
    #[auto]
    id: i64,
    text: String,
}
impl Baz {
    fn new(text: &str) -> Self {
        Baz {
            id: -1, // will be set automatically when saved
            text: text.to_string(),
            state: ObjectState::default(),
        }
    }
}

#[model]
struct HasOnlyPk {
    id: i64,
}
impl HasOnlyPk {
    fn new(id: i64) -> Self {
        HasOnlyPk {
            id,
            state: ObjectState::default(),
        }
    }
}

#[model]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelfReferential {
    pub id: i32,
    pub reference: Option<ForeignKey<SelfReferential>>,
}
impl SelfReferential {
    fn new(id: i32) -> Self {
        SelfReferential {
            id,
            reference: None,
            state: ObjectState::default(),
        }
    }
}

#[model]
#[derive(Debug, Default, PartialEq, Clone)]
struct TimeHolder {
    pub id: i32,
    pub naive: NaiveDateTime,
    pub utc: DateTime<Utc>,
    pub when: chrono::DateTime<Utc>,
}

fn basic_crud(conn: Connection) {
    //create
    let mut foo = Foo::new(1);
    foo.bam = 0.1;
    foo.bar = 42;
    foo.baz = "hello world".to_string();
    foo.blobbity = [1u8, 2u8, 3u8].to_vec();
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo, foo2);
    assert_eq!(Some(foo), Foo::try_get(&conn, 1).unwrap());

    // update
    foo2.bam = 0.2;
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
    let foo3 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo2, foo3);

    // delete
    assert!(foo3.delete(&conn).is_ok());
    if let Some(butane::Error::NoSuchObject) = Foo::get(&conn, 1).err() {
    } else {
        panic!("Expected NoSuchObject");
    }
    assert_eq!(None, Foo::try_get(&conn, 1).unwrap());
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
    let found: Foo = find!(Foo, bar == 43, &conn).unwrap();
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

fn basic_query_delete(conn: Connection) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).unwrap();
    let mut foo3 = Foo::new(3);
    foo3.bar = 44;
    foo3.baz = "goodbye world".to_string();
    foo3.save(&conn).unwrap();

    // delete just the last one
    let cnt = query!(Foo, baz == "goodbye world").delete(&conn).unwrap();
    assert_eq!(cnt, 1);

    // delete the other two
    let cnt = query!(Foo, baz.like("hello%")).delete(&conn).unwrap();
    assert_eq!(cnt, 2);
}
testall!(basic_query_delete);

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

fn auto_pk(conn: Connection) {
    let mut baz1 = Baz::new("baz1");
    baz1.save(&conn).unwrap();
    let mut baz2 = Baz::new("baz2");
    baz2.save(&conn).unwrap();
    let mut baz3 = Baz::new("baz3");
    baz3.save(&conn).unwrap();
    assert!(baz1.id < baz2.id);
    assert!(baz2.id < baz3.id);
}
testall!(auto_pk);

fn only_pk(conn: Connection) {
    let mut obj = HasOnlyPk::new(1);
    obj.save(&conn).unwrap();
    // verify we can still save the object even though it has no
    // fields to modify
    obj.save(&conn).unwrap();
}
testall!(only_pk);

fn basic_committed_transaction(mut conn: Connection) {
    let tr = conn.transaction().unwrap();

    // Create an object with a transaction and commit it
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.save(&tr).unwrap();
    tr.commit().unwrap();

    // Find the object
    let foo2 = Foo::get(&conn, 1).unwrap();
    assert_eq!(foo, foo2);
}
testall!(basic_committed_transaction);

fn basic_dropped_transaction(mut conn: Connection) {
    // Create an object with a transaction but never commit it
    {
        let tr = conn.transaction().unwrap();
        let mut foo = Foo::new(1);
        foo.bar = 42;
        foo.save(&tr).unwrap();
    }

    // Find the object
    match Foo::get(&conn, 1) {
        Ok(_) => panic!("object should not exist"),
        Err(butane::Error::NoSuchObject) => (),
        Err(e) => panic!("Unexpected error {e}"),
    }
}
testall!(basic_dropped_transaction);

fn basic_rollback_transaction(mut conn: Connection) {
    let tr = conn.transaction().unwrap();

    // Create an object with a transaction but then roll back the transaction
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.save(&tr).unwrap();
    tr.rollback().unwrap();

    // Find the object
    match Foo::get(&conn, 1) {
        Ok(_) => panic!("object should not exist"),
        Err(butane::Error::NoSuchObject) => (),
        Err(e) => panic!("Unexpected error {e}"),
    }
}
testall!(basic_rollback_transaction);

fn basic_unique_field_error_on_non_unique(conn: Connection) {
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.save(&conn).unwrap();

    let mut foo2 = Foo::new(2);
    foo2.bar = foo1.bar;
    let e = foo2.save(&conn).unwrap_err();
    // Make sure the error is one we expect
    assert!(match e {
        #[cfg(feature = "sqlite")]
        butane::Error::SQLite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error { code, .. },
            _,
        )) if code == rusqlite::ffi::ErrorCode::ConstraintViolation => true,
        #[cfg(feature = "pg")]
        butane::Error::Postgres(e)
            if e.code() == Some(&postgres::error::SqlState::UNIQUE_VIOLATION) =>
            true,
        _ => false,
    });
}
testall!(basic_unique_field_error_on_non_unique);

fn fkey_same_type(conn: Connection) {
    let mut o1 = SelfReferential::new(1);
    let mut o2 = SelfReferential::new(2);
    o2.save(&conn).unwrap();
    o1.reference = Some(ForeignKey::from_pk(o2.id));
    o1.save(&conn).unwrap();

    let o1 = SelfReferential::get(&conn, 1).unwrap();
    assert!(o1.reference.is_some());
    let inner: SelfReferential = o1.reference.unwrap().load(&conn).unwrap().clone();
    assert_eq!(inner, o2);
    assert!(inner.reference.is_none());
}
testall!(fkey_same_type);

fn basic_time(conn: Connection) {
    let now = Utc::now();
    let mut time = TimeHolder {
        id: 1,
        naive: now.naive_utc(),
        utc: now,
        when: now,
        state: ObjectState::default(),
    };
    time.save(&conn).unwrap();

    let time2 = TimeHolder::get(&conn, 1).unwrap();
    // Note, we don't just compare the objects directly because we
    // lose some precision when we go to the database.
    assert_eq!(time.naive.timestamp(), time2.naive.timestamp());
}
testall!(basic_time);

fn basic_load_first(conn: Connection) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).unwrap();

    // query finds first
    let found = query!(Foo, baz.like("hello%")).load_first(&conn).unwrap();

    assert_eq!(found, Some(foo1));
}
testall!(basic_load_first);

fn basic_load_first_ordered(conn: Connection) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).unwrap();

    // query finds first, ascending order
    let found_asc = query!(Foo, baz.like("hello%"))
        .order_asc(colname!(Foo, bar))
        .load_first(&conn)
        .unwrap();

    assert_eq!(found_asc, Some(foo1));

    // query finds first, descending order
    let found_desc = query!(Foo, baz.like("hello%"))
        .order_desc(colname!(Foo, bar))
        .load_first(&conn)
        .unwrap();

    assert_eq!(found_desc, Some(foo2));
}
testall!(basic_load_first_ordered);
