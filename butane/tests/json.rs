use std::collections::HashMap;

use butane::model;
use butane::prelude::*;
use butane::{db::Connection, FieldType, ObjectState};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use butane_test_helper::*;

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct FooJJ {
    id: i64,
    val: serde_json::Value,
    bar: u32,
}
impl FooJJ {
    fn new(id: i64) -> Self {
        FooJJ {
            id,
            val: Value::default(),
            bar: 0,
            state: ObjectState::default(),
        }
    }
}

fn json_null(conn: Connection) {
    //create
    let id = 4;
    let mut foo = FooJJ::new(id);
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = FooJJ::get(&conn, id).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
    let foo3 = FooJJ::get(&conn, id).unwrap();
    assert_eq!(foo2, foo3);
}
testall!(json_null);

fn basic_json(conn: Connection) {
    //create
    let id = 4;
    let mut foo = FooJJ::new(id);
    let data = r#"
        {
            "name": "John Doe",
            "age": 43,
            "phones": [
                "+44 1234567",
                "+44 2345678"
            ]
        }"#;

    foo.val = serde_json::from_str(data).unwrap();
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = FooJJ::get(&conn, id).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
    let foo3 = FooJJ::get(&conn, id).unwrap();
    assert_eq!(foo2, foo3);
}
testall!(basic_json);

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct FooHH {
    id: i64,
    val: HashMap<String, String>,
    bar: u32,
}
impl FooHH {
    fn new(id: i64) -> Self {
        FooHH {
            id,
            val: HashMap::<String, String>::default(),
            bar: 0,
            state: ObjectState::default(),
        }
    }
}
fn basic_hashmap(conn: Connection) {
    //create
    let id = 4;
    let mut foo = FooHH::new(id);
    let mut data = HashMap::<String, String>::new();
    data.insert("a".to_string(), "1".to_string());

    foo.val = data;
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = FooHH::get(&conn, id).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
    let foo3 = FooHH::get(&conn, id).unwrap();
    assert_eq!(foo2, foo3);
}
testall!(basic_hashmap);

#[derive(PartialEq, Eq, Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
struct HashedObject {
    x: i64,
    y: i64,
}

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct FooHHO {
    id: i64,
    val: HashMap<String, HashedObject>,
    bar: u32,
}
impl FooHHO {
    fn new(id: i64) -> Self {
        FooHHO {
            id,
            val: HashMap::<String, HashedObject>::default(),
            bar: 0,
            state: ObjectState::default(),
        }
    }
}
fn hashmap_with_object_values(conn: Connection) {
    //create
    let id = 4;
    let mut foo = FooHHO::new(id);
    let mut data = HashMap::<String, HashedObject>::new();
    data.insert("a".to_string(), HashedObject { x: 1, y: 3 });

    foo.val = data;
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = FooHHO::get(&conn, id).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = 43;
    foo2.save(&conn).unwrap();
    let foo3 = FooHHO::get(&conn, id).unwrap();
    assert_eq!(foo2, foo3);
}
testall!(hashmap_with_object_values);

#[derive(PartialEq, Eq, Debug, Clone, FieldType, Serialize, Deserialize)]
struct InlineFoo {
    foo: i64,
    bar: u32,
}
impl InlineFoo {
    fn new(foo: i64, bar: u32) -> Self {
        InlineFoo { foo, bar }
    }
}

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct OuterFoo {
    #[pk]
    id: i64,
    bar: InlineFoo,
}
impl OuterFoo {
    fn new(id: i64, bar: InlineFoo) -> Self {
        OuterFoo {
            id,
            bar,
            state: ObjectState::default(),
        }
    }
}

fn inline_json(conn: Connection) {
    //create
    let id = 4;
    let mut foo = OuterFoo::new(id, InlineFoo::new(4, 8));
    foo.save(&conn).unwrap();

    // read
    let mut foo2 = OuterFoo::get(&conn, id).unwrap();
    assert_eq!(foo, foo2);

    // update
    foo2.bar = InlineFoo::new(5, 9);
    foo2.save(&conn).unwrap();
    let foo3 = OuterFoo::get(&conn, id).unwrap();
    assert_eq!(foo2, foo3);
}
testall!(inline_json);
