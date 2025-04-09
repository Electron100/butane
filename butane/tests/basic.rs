#![allow(clippy::disallowed_names)]

use butane::colname;
use butane::db::{Connection, ConnectionAsync};
use butane::{butane_type, find, find_async, model, query, AutoPk, ForeignKey};
use butane_test_helper::*;
use butane_test_macros::butane_test;
#[cfg(feature = "datetime")]
use chrono::{naive::NaiveDateTime, offset::Utc, DateTime};
#[cfg(feature = "sqlite")]
use rusqlite;
use std::ops::Deref;
#[cfg(feature = "pg")]
use tokio_postgres as postgres;

#[butane_type]
pub type Whatsit = String;

// Note, Serialize derive exists solely to exercise the logic in butane_core::codegen::has_derive_serialize
#[model]
#[derive(PartialEq, Debug, Clone, serde::Serialize)]
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

#[model]
struct Baz {
    id: AutoPk<i64>,
    text: String,
}
impl Baz {
    fn new(text: &str) -> Self {
        Baz {
            id: AutoPk::default(),
            text: text.to_string(),
        }
    }
}

#[model]
struct HasOnlyPk {
    id: i64,
}
impl HasOnlyPk {
    fn new(id: i64) -> Self {
        HasOnlyPk { id }
    }
}

#[model]
#[derive(Default)]
struct HasOnlyAutoPk {
    id: AutoPk<i64>,
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
        }
    }
}

#[cfg(feature = "datetime")]
#[model]
#[derive(Debug, Default, PartialEq, Clone)]
struct TimeHolder {
    pub id: i32,
    pub naive: NaiveDateTime,
    pub utc: DateTime<Utc>,
    pub when: chrono::DateTime<Utc>,
}

#[butane_test]
async fn basic_crud(conn: ConnectionAsync) {
    //create
    let mut foo = Foo::new(1);
    foo.bam = 0.1;
    foo.bar = 42;
    foo.baz = "hello world".to_string();
    foo.blobbity = [1u8, 2u8, 3u8].to_vec();
    foo.save(&conn).await.unwrap();

    // read
    let mut foo2 = Foo::get(&conn, 1).await.unwrap();
    assert_eq!(foo, foo2);
    assert_eq!(Some(foo), Foo::try_get(&conn, 1).await.unwrap());

    // update
    foo2.bam = 0.2;
    foo2.bar = 43;
    foo2.save(&conn).await.unwrap();
    let foo3 = Foo::get(&conn, 1).await.unwrap();
    assert_eq!(foo2, foo3);

    // delete
    assert!(foo3.delete(&conn).await.is_ok());
    match Foo::get(&conn, 1).await.err() {
        Some(butane::Error::NoSuchObject) => (),
        _ => panic!("Expected NoSuchObject"),
    }
    assert_eq!(None, Foo::try_get(&conn, 1).await.unwrap());
}

#[butane_test]
async fn basic_find(conn: ConnectionAsync) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).await.unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).await.unwrap();

    // find
    let found: Foo = find_async!(Foo, bar == 43, &conn).unwrap();
    assert_eq!(found, foo2);
}

#[butane_test]
async fn basic_query(conn: ConnectionAsync) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).await.unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).await.unwrap();

    // query finds 1
    let mut found = query!(Foo, bar == 42).load(&conn).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found.pop().unwrap(), foo1);

    // query finds both
    let found = query!(Foo, bar < 44).load(&conn).await.unwrap();
    assert_eq!(found.len(), 2);
}

#[butane_test]
async fn basic_query_delete(conn: ConnectionAsync) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).await.unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).await.unwrap();
    let mut foo3 = Foo::new(3);
    foo3.bar = 44;
    foo3.baz = "goodbye world".to_string();
    foo3.save(&conn).await.unwrap();

    // delete just the last one
    let cnt = query!(Foo, baz == "goodbye world")
        .delete(&conn)
        .await
        .unwrap();
    assert_eq!(cnt, 1);

    // delete the other two
    let cnt = query!(Foo, baz.like("hello%")).delete(&conn).await.unwrap();
    assert_eq!(cnt, 2);
}

#[butane_test]
async fn string_pk(conn: ConnectionAsync) {
    let mut foo = Foo::new(1);
    foo.save(&conn).await.unwrap();
    let mut bar = Bar::new("tarzan", foo);
    bar.save(&conn).await.unwrap();

    let bar2 = Bar::get(&conn, "tarzan".to_string()).await.unwrap();
    assert_eq!(bar, bar2);
}

#[butane_test]
async fn foreign_key(conn: ConnectionAsync) {
    let mut foo = Foo::new(1);
    foo.save(&conn).await.unwrap();
    let mut bar = Bar::new("tarzan", foo.clone());
    bar.save(&conn).await.unwrap();
    let bar2 = Bar::get(&conn, "tarzan".to_string()).await.unwrap();

    let foo2: &Foo = bar2.foo.load(&conn).await.unwrap();
    assert_eq!(&foo, foo2);

    let foo3: &Foo = bar2.foo.get().unwrap();
    assert_eq!(foo2, foo3);
}

#[butane_test]
async fn auto_pk(conn: ConnectionAsync) {
    let mut baz1 = Baz::new("baz1");
    baz1.save(&conn).await.unwrap();
    let mut baz2 = Baz::new("baz2");
    baz2.save(&conn).await.unwrap();
    let mut baz3 = Baz::new("baz3");
    baz3.save(&conn).await.unwrap();
    assert!(baz1.id < baz2.id);
    assert!(baz2.id < baz3.id);
    let _raw_pk: i64 = baz1.id.deref().unwrap();
}

#[butane_test]
async fn only_pk(conn: ConnectionAsync) {
    let mut obj = HasOnlyPk::new(1);
    obj.save(&conn).await.unwrap();
    assert_eq!(obj.id, 1);
    // verify we can still save the object even though it has no
    // fields to modify
    obj.save(&conn).await.unwrap();
    // verify it didnt get a new id
    assert_eq!(obj.id, 1);
}

#[butane_test]
async fn only_auto_pk(conn: ConnectionAsync) {
    let mut obj = HasOnlyAutoPk::default();
    obj.save(&conn).await.unwrap();
    let pk = obj.id;
    // verify we can still save the object even though it has no
    // fields to modify
    obj.save(&conn).await.unwrap();
    // verify it didnt get a new id
    assert_eq!(obj.id, pk);
}

#[butane_test]
async fn basic_committed_transaction(mut conn: ConnectionAsync) {
    let tr = conn.transaction().await.unwrap();

    // Create an object with a transaction and commit it
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.save(&tr).await.unwrap();
    tr.commit().await.unwrap();

    // Find the object
    let foo2 = Foo::get(&conn, 1).await.unwrap();
    assert_eq!(foo, foo2);
}

#[butane_test]
async fn basic_dropped_transaction(mut conn: ConnectionAsync) {
    // Create an object with a transaction but never commit it
    {
        let tr = conn.transaction().await.unwrap();
        let mut foo = Foo::new(1);
        foo.bar = 42;
        foo.save(&tr).await.unwrap();
    }

    // Find the object
    match Foo::get(&conn, 1).await {
        Ok(_) => panic!("object should not exist"),
        Err(butane::Error::NoSuchObject) => (),
        Err(e) => panic!("Unexpected error {e}"),
    }
}

#[butane_test]
async fn basic_rollback_transaction(mut conn: ConnectionAsync) {
    let tr = conn.transaction().await.unwrap();

    // Create an object with a transaction but then roll back the transaction
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.save(&tr).await.unwrap();
    tr.rollback().await.unwrap();

    // Find the object
    match Foo::get(&conn, 1).await {
        Ok(_) => panic!("object should not exist"),
        Err(butane::Error::NoSuchObject) => (),
        Err(e) => panic!("Unexpected error {e}"),
    }
}

#[butane_test]
async fn basic_unique_field_error_on_non_unique(conn: ConnectionAsync) {
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.save(&conn).await.unwrap();

    let mut foo2 = Foo::new(2);
    foo2.bar = foo1.bar;
    let e = foo2.save(&conn).await.unwrap_err();
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

#[butane_test]
async fn fkey_same_type(conn: ConnectionAsync) {
    let mut o1 = SelfReferential::new(1);
    let mut o2 = SelfReferential::new(2);
    o2.save(&conn).await.unwrap();
    o1.reference = Some(ForeignKey::from_pk(o2.id));
    o1.save(&conn).await.unwrap();

    let o1 = SelfReferential::get(&conn, 1).await.unwrap();
    assert!(o1.reference.is_some());
    let inner: SelfReferential = o1.reference.unwrap().load(&conn).await.unwrap().clone();
    assert_eq!(inner, o2);
    assert!(inner.reference.is_none());
}

#[butane_test]
async fn cant_save_unsaved_fkey(conn: ConnectionAsync) {
    let foo = Foo::new(1);
    let mut bar = Bar::new("tarzan", foo);
    assert!(bar.save(&conn).await.is_err());
}

#[cfg(feature = "datetime")]
#[butane_test]
async fn basic_time(conn: ConnectionAsync) {
    let mut now = Utc::now();
    // Ensure we have a non-zero subsecond value so that the assert below confirms
    // that the time is actually being saved with subsecond precision.
    while now.timestamp_subsec_nanos() == 0 {
        now = Utc::now();
    }

    let mut time = TimeHolder {
        id: 1,
        naive: now.naive_utc(),
        utc: now,
        when: now,
    };
    time.save(&conn).await.unwrap();

    let time2 = TimeHolder::get(&conn, 1).await.unwrap();
    if conn.backend_name() == "sqlite" {
        assert_eq!(time.utc, time2.utc);
    } else {
        assert_eq!(time.utc.timestamp_micros(), time2.utc.timestamp_micros());
    }
}

#[butane_test]
async fn basic_load_first(conn: ConnectionAsync) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).await.unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).await.unwrap();

    // query finds first
    let found = query!(Foo, baz.like("hello%"))
        .load_first(&conn)
        .await
        .unwrap();

    assert_eq!(found, Some(foo1));
}

#[butane_test]
async fn basic_load_first_ordered(conn: ConnectionAsync) {
    //create
    let mut foo1 = Foo::new(1);
    foo1.bar = 42;
    foo1.baz = "hello world".to_string();
    foo1.save(&conn).await.unwrap();
    let mut foo2 = Foo::new(2);
    foo2.bar = 43;
    foo2.baz = "hello world".to_string();
    foo2.save(&conn).await.unwrap();

    // query finds first, ascending order
    let found_asc = query!(Foo, baz.like("hello%"))
        .order_asc(colname!(Foo, bar))
        .load_first(&conn)
        .await
        .unwrap();

    assert_eq!(found_asc, Some(foo1));

    // query finds first, descending order
    let found_desc = query!(Foo, baz.like("hello%"))
        .order_desc(colname!(Foo, bar))
        .load_first(&conn)
        .await
        .unwrap();

    assert_eq!(found_desc, Some(foo2));
}

#[butane_test]
async fn save_upserts_by_default(conn: ConnectionAsync) {
    let mut foo = Foo::new(1);
    foo.bar = 42;
    foo.save(&conn).await.unwrap();

    // Create another foo object with the same primary key,
    // but a different bar value.
    let mut foo = Foo::new(1);
    foo.bar = 43;
    // Save should do an upsert, so it will update the bar value
    // rather than throwing a conflict
    foo.save(&conn).await.unwrap();

    let retrieved = Foo::get(&conn, 1).await.unwrap();
    assert_eq!(retrieved.bar, 43);
}

#[butane_test(async)]
async fn tokio_spawn(conn: ConnectionAsync) {
    // This test exists mostly to make sure it compiles. Verifies that
    // we can Send the futures from ConnectionMethodsAsync.
    tokio::spawn(async move {
        let mut foo = Foo::new(1);
        foo.save(&conn).await.unwrap();
    });
}

#[model]
struct TypeVariantsTest {
    id: i64,
    str1: String,
    str2: std::string::String,
    str3: ::std::string::String,
    blob1: Vec<u8>,
    blob2: std::vec::Vec<u8>,
    blob3: ::std::vec::Vec<u8>,
}
