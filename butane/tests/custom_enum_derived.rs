// Tests deriving FieldType for an enum
use butane::db::Connection;
use butane::prelude::*;
use butane::{model, query};
use butane::{FieldType, FromSql, SqlVal, ToSql};
use butane_test_helper::*;

#[derive(PartialEq, Eq, Debug, Clone, FieldType)]
enum Whatsit {
    Foo,
    Bar,
    Baz,
}

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct HasCustomField2 {
    id: i64,
    frob: Whatsit,
}
impl HasCustomField2 {
    fn new(id: i64, frob: Whatsit) -> Self {
        HasCustomField2 { id, frob }
    }
}

async fn roundtrip_custom_type(conn: Connection) {
    //create
    let mut obj = HasCustomField2::new(1, Whatsit::Foo);
    obj.save(&conn).await.unwrap();

    // read
    let obj2 = HasCustomField2::get(&conn, 1).await.unwrap();
    assert_eq!(obj, obj2);
}
testall!(roundtrip_custom_type);

async fn query_custom_type(conn: Connection) {
    //create
    let mut obj_foo = HasCustomField2::new(1, Whatsit::Foo);
    obj_foo.save(&conn).await.unwrap();
    let mut obj_bar = HasCustomField2::new(2, Whatsit::Bar);
    obj_bar.save(&conn).await.unwrap();

    // query
    let results = query!(HasCustomField2, frob == { Whatsit::Bar })
        .load(&conn)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], obj_bar)
}
testall!(query_custom_type);

#[test]
fn enum_to_sql() {
    assert_eq!(SqlVal::Text("Foo".to_string()), Whatsit::Foo.to_sql());
    assert_eq!(SqlVal::Text("Bar".to_string()), Whatsit::Bar.to_sql());
    assert_eq!(SqlVal::Text("Baz".to_string()), Whatsit::Baz.to_sql());
}

#[test]
fn enum_from_sql() {
    assert_eq!(
        Whatsit::Foo,
        Whatsit::from_sql(SqlVal::Text("Foo".to_string())).unwrap()
    );
    assert_eq!(
        Whatsit::Bar,
        Whatsit::from_sql(SqlVal::Text("Bar".to_string())).unwrap()
    );
    assert_eq!(
        Whatsit::Baz,
        Whatsit::from_sql(SqlVal::Text("Baz".to_string())).unwrap()
    );
    match Whatsit::from_sql(SqlVal::Text("Nope".to_string())) {
        Ok(_) => panic!("Not a valid enum variant"),
        Err(butane::Error::UnknownEnumVariant(_)) => {} // OK
        Err(_) => panic!("Unexpected error"),
    }
}
