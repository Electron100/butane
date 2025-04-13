//! Demonstrates use of non-standard Postgres types. See README for more.

use butane::db::ConnectionAsync;
use butane::prelude_async::*;
use butane::AutoPk;
use butane_test_macros::butane_test;

mod models;
use models::{Point, Trip};

/// Saves and loads a `Trip`.
pub async fn roundtrip_trip_through_db(conn: ConnectionAsync) {
    let mut trip = Trip {
        id: AutoPk::uninitialized(),
        pt_from: Point::new(0.0, 0.0),
        pt_to: Point::new(8.0, 9.0),
    };
    trip.save(&conn).await.unwrap();

    let trip2 = Trip::get(&conn, trip.id).await.unwrap();
    assert_eq!(trip, trip2);
}

/// Tests saving and loading a custom type.
#[butane_test(async, pg)]
async fn roundtrip_custom(conn: ConnectionAsync) {
    crate::roundtrip_trip_through_db(conn).await;
}

// TODO point in postgres doesn't support normal equality
/*#[butane_test(async, pg)]
async fn query_custom(conn: ConnectionAsync) {
    use butane::query;
    let origin = Point::new(0.0, 0.0);
    let mut trip1 = Trip {
        id: AutoPk::default(),
        pt_from: origin.clone(),
        pt_to: Point::new(8.0, 9.0),
    };
    trip1.save(&conn).await.unwrap();

    let mut trip2 = Trip {
        id: AutoPk::default(),
        pt_from: Point::new(1.1, 2.0),
        pt_to: Point::new(7.0, 6.0),
    };
    trip2.save(&conn).await.unwrap();

    let trips = query!(Trip, pt_from == { origin })
        .load(&conn)
        .await
        .unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(trip1, trips[0]);
}*/
