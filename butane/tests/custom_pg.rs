// We wrap everything in an inner module just so it's easier to have the feature gate in one place
#[cfg(feature = "pg")]
mod custom_pg {
    use butane::custom::{SqlTypeCustom, SqlValRefCustom};
    use butane::prelude::*;
    use butane::{butane_type, db::Connection, model, ObjectState};
    use butane::{FieldType, FromSql, SqlType, SqlVal, SqlValRef, ToSql};
    use butane_test_helper::{maketest, maketest_pg};

    use std::result::Result;

    // newtype so we can implement traits for it.
    #[butane_type(Custom(POINT))]
    #[derive(Debug, PartialEq, Clone)]
    struct Point(geo_types::Point<f64>);
    impl Point {
        fn new(x: f64, y: f64) -> Self {
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

    #[model]
    #[derive(Clone, Debug, PartialEq)]
    struct Trip {
        #[auto]
        id: i64,
        pt_from: Point,
        pt_to: Point,
    }

    fn roundtrip_custom(conn: Connection) {
        let mut trip = Trip {
            id: -1,
            pt_from: Point::new(0.0, 0.0),
            pt_to: Point::new(8.0, 9.0),
            state: ObjectState::default(),
        };
        trip.save(&conn).unwrap();

        let trip2 = Trip::get(&conn, trip.id).unwrap();
        assert_eq!(trip, trip2);
    }
    maketest_pg!(roundtrip_custom, true);

    /*
        TODO point in postgres doesn't support normal equality, so need
        fn query_custom(conn: Connection) {
        let origin = Point::new(0.0, 0.0);
        let mut trip1 = Trip {
            id: -1,
            pt_from: origin.clone(),
            pt_to: Point::new(8.0, 9.0),
            state: ObjectState::default(),
        };
        trip1.save(&conn).unwrap();

        let mut trip2 = Trip {
            id: -1,
            pt_from: Point::new(1.1, 2.0),
            pt_to: Point::new(7.0, 6.0),
            state: ObjectState::default(),
        };
        trip2.save(&conn).unwrap();

        let trips = query!(Trip, pt_from ~= { origin }).load(&conn).unwrap();
        assert_eq!(trips.len(), 1);
        assert_eq!(trip1, trips[0]);
    }

    maketest_pg!(query_custom);*/
}
