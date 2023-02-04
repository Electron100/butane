use butane::model;
use butane::prelude::*;
use butane::{db::Connection, ObjectState};
use serde_json::Value;

mod common;

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct FooJJ {
    id: i64,
    val: Value,
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
