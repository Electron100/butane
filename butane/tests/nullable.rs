use butane::db::Connection;
use butane::prelude::*;
use butane::{model, query};
use butane_test_helper::*;

#[model]
#[derive(PartialEq, Eq, Debug)]
struct WithNullable {
    id: i64,
    foo: Option<i32>,
}
impl WithNullable {
    fn new(id: i64) -> Self {
        WithNullable { id, foo: None }
    }
}

async fn basic_optional(conn: Connection) {
    let mut with_none = WithNullable::new(1);
    with_none.save(&conn).await.unwrap();

    let mut with_some = WithNullable::new(2);
    with_some.foo = Some(42);
    with_some.save(&conn).await.unwrap();

    let obj = WithNullable::get(&conn, 1).await.unwrap();
    assert_eq!(obj.foo, None);

    let obj = WithNullable::get(&conn, 2).await.unwrap();
    assert_eq!(obj.foo, Some(42));
}
testall!(basic_optional);

async fn query_optional_with_some(conn: Connection) {
    let mut obj = WithNullable::new(1);
    obj.save(&conn).await.unwrap();

    let mut obj = WithNullable::new(2);
    obj.foo = Some(42);
    obj.save(&conn).await.unwrap();

    let mut obj = WithNullable::new(3);
    obj.foo = Some(43);
    obj.save(&conn).await.unwrap();

    let mut obj = WithNullable::new(4);
    obj.foo = Some(44);
    obj.save(&conn).await.unwrap();

    let mut objs = query!(WithNullable, foo > 42).load(&conn).await.unwrap();
    objs.sort_by(|o1, o2| o1.foo.partial_cmp(&o2.foo).unwrap());
    assert_eq!(objs.len(), 2);
    assert_eq!(objs[0].foo, Some(43));
    assert_eq!(objs[1].foo, Some(44));
}
testall!(query_optional_with_some);

async fn query_optional_with_none(conn: Connection) {
    let mut obj = WithNullable::new(1);
    obj.save(&conn).await.unwrap();

    let mut obj = WithNullable::new(2);
    obj.foo = Some(42);
    obj.save(&conn).await.unwrap();

    let objs = query!(WithNullable, foo == None).load(&conn).await.unwrap();
    assert_eq!(objs.len(), 1);
    assert_eq!(objs[0].id, 1);
}
testall!(query_optional_with_none);
