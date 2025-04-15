use butane::custom::{SqlTypeCustom, SqlValRefCustom};
use butane::{butane_type, model};
use butane::{AutoPk, FieldType, FromSql, SqlType, SqlVal, SqlValRef, ToSql};
use tokio_postgres as postgres;

/// Newtype for geo_types::Point so we can implement traits for it.
#[butane_type(Custom(POINT))]
#[derive(Debug, PartialEq, Clone)]
pub struct Point(geo_types::Point<f64>);
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point(geo_types::Point::<f64>::new(x, y))
    }
}

impl ToSql for Point {
    fn to_sql_ref(&self) -> SqlValRef<'_> {
        SqlValRef::Custom(SqlValRefCustom::PgToSql {
            ty: postgres::types::Type::POINT,
            tosql: &self.0,
        })
    }
    fn to_sql(&self) -> SqlVal {
        self.to_sql_ref().into()
    }
}

impl FromSql for Point {
    fn from_sql_ref(val: SqlValRef) -> Result<Self, butane::Error> {
        match val {
            SqlValRef::Custom(SqlValRefCustom::PgBytes { ty, data }) => {
                Ok(Point(postgres::types::FromSql::from_sql(&ty, data)?))
            }
            _ => Err(butane::Error::CannotConvertSqlVal(
                Point::SQLTYPE,
                val.into(),
            )),
        }
    }
}

impl FieldType for Point {
    const SQLTYPE: SqlType = SqlType::Custom(SqlTypeCustom::Pg(postgres::types::Type::LINE));
    type RefType = Self;
}

/// Represents a trip from one point to another.
#[model]
#[derive(Debug, PartialEq)]
pub struct Trip {
    pub id: AutoPk<i64>,
    pub pt_from: Point,
    pub pt_to: Point,
}
