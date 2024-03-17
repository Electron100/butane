use butane::db::Connection;
use butane::prelude::*;
use butane::{butane_type, model, query};
use butane::{FieldType, FromSql, SqlType, SqlVal, SqlValRef, ToSql};
use butane_test_helper::*;

#[butane_type(Text)]
#[derive(PartialEq, Eq, Debug, Clone)]
enum Frobnozzle {
    Foo,
    Bar,
    Baz,
}

impl ToSql for Frobnozzle {
    fn to_sql(&self) -> SqlVal {
        self.to_sql_ref().into()
    }
    fn to_sql_ref(&self) -> SqlValRef<'_> {
        SqlValRef::Text(match self {
            Frobnozzle::Foo => "Foo",
            Frobnozzle::Bar => "Bar",
            Frobnozzle::Baz => "Baz",
        })
    }
}
impl FromSql for Frobnozzle {
    fn from_sql_ref(val: SqlValRef) -> Result<Self, butane::Error> {
        match val {
            SqlValRef::Text(s) => match s {
                "Foo" => Ok(Self::Foo),
                "Bar" => Ok(Self::Bar),
                "Baz" => Ok(Self::Baz),
                _ => Err(butane::Error::CannotConvertSqlVal(
                    SqlType::Text,
                    val.into(),
                )),
            },
            _ => Err(butane::Error::CannotConvertSqlVal(
                SqlType::Text,
                val.into(),
            )),
        }
    }
}
impl FieldType for Frobnozzle {
    type RefType = Self;
    const SQLTYPE: SqlType = SqlType::Text;
}

#[model]
#[derive(PartialEq, Eq, Debug, Clone)]
struct HasCustomField {
    id: i64,
    frob: Frobnozzle,
}
impl HasCustomField {
    fn new(id: i64, frob: Frobnozzle) -> Self {
        HasCustomField { id, frob }
    }
}

async fn roundtrip_custom_type(conn: Connection) {
    //create
    let mut obj = HasCustomField::new(1, Frobnozzle::Foo);
    obj.save(&conn).await.unwrap();

    // read
    let obj2 = HasCustomField::get(&conn, 1).await.unwrap();
    assert_eq!(obj, obj2);
}
testall!(roundtrip_custom_type);

async fn query_custom_type(conn: Connection) {
    //create
    let mut obj_foo = HasCustomField::new(1, Frobnozzle::Foo);
    obj_foo.save(&conn).await.unwrap();
    let mut obj_bar = HasCustomField::new(2, Frobnozzle::Bar);
    obj_bar.save(&conn).await.unwrap();

    // query
    let results = query!(HasCustomField, frob == { Frobnozzle::Bar })
        .load(&conn)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], obj_bar)
}
testall!(query_custom_type);
