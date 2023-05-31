#[cfg(feature = "r2d2")]
use butane_test_helper::{pg_connspec, setup_db, sqlite_connspec};

#[cfg(feature = "r2d2")]
use butane::db;
#[cfg(feature = "r2d2")]
use r2d2_for_test as r2d2;

#[cfg(all(feature = "sqlite", feature = "r2d2"))]
#[test]
fn r2d2_sqlite() {
    let manager = db::ConnectionManager::new(sqlite_connspec());
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        setup_db(
            Box::new(butane::db::sqlite::SQLiteBackend::new()),
            &mut conn1,
            true,
        );

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}

#[cfg(all(feature = "pg", feature = "r2d2"))]
#[test]
fn r2d2_pq() {
    let (connspec, _data) = pg_connspec();
    let manager = db::ConnectionManager::new(connspec);
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        setup_db(Box::new(butane::db::pg::PgBackend::new()), &mut conn1, true);

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}
