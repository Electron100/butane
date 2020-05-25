use paste;
use propane::db::Connection;
use propane::prelude::*;
use propane::{model, propane_type, query};
use propane::{FieldType, FromSql, IntoSql, ObjectState, SqlType, SqlVal, ToSql};

mod common;

#[propane_type(Text)]
#[derive(PartialEq, Eq, Debug, Clone)]
enum Frobnozzle {
    Foo,
    Bar,
    Baz,
}

impl ToSql for Frobnozzle {
    fn to_sql(&self) -> SqlVal {
        SqlVal::Text(
            match self {
                Frobnozzle::Foo => "Foo",
                Frobnozzle::Bar => "Bar",
                Frobnozzle::Baz => "Baz",
            }
            .to_string(),
        )
    }
}
impl IntoSql for Frobnozzle {
    fn into_sql(self) -> SqlVal {
        self.to_sql()
    }
}
impl FromSql for Frobnozzle {
    fn from_sql(val: SqlVal) -> Result<Self, propane::Error> {
        match val {
            SqlVal::Text(ref s) => match s.as_ref() {
                "Foo" => Ok(Self::Foo),
                "Bar" => Ok(Self::Bar),
                "Baz" => Ok(Self::Baz),
                _ => Err(propane::Error::CannotConvertSqlVal(
                    SqlType::Text,
                    val.clone(),
                )),
            },
            _ => Err(propane::Error::CannotConvertSqlVal(
                SqlType::Text,
                val.clone(),
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
        HasCustomField {
            id,
            frob,
            state: ObjectState::default(),
        }
    }
}

fn roundtrip_custom_type(conn: Connection) {
    //create
    let mut obj = HasCustomField::new(1, Frobnozzle::Foo);
    obj.save(&conn).unwrap();

    // read
    let obj2 = HasCustomField::get(&conn, 1).unwrap();
    assert_eq!(obj, obj2);
}
testall!(roundtrip_custom_type);

fn query_custom_type(conn: Connection) {
    //create
    let mut obj_foo = HasCustomField::new(1, Frobnozzle::Foo);
    obj_foo.save(&conn).unwrap();
    let mut obj_bar = HasCustomField::new(2, Frobnozzle::Bar);
    obj_bar.save(&conn).unwrap();

    // query
    let results = query!(HasCustomField, frob == { Frobnozzle::Bar })
        .load(&conn)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], obj_bar)
}
testall!(query_custom_type);
