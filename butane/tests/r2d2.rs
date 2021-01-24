mod common;
#[cfg(feature = "r2d2")]
use butane::db;

#[cfg(all(feature = "sqlite", feature = "r2d2"))]
#[test]
fn r2d2_sqlite() {
    let manager = db::ConnectionManager::new(common::sqlite_connspec());
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        common::setup_db(
            Box::new(butane::db::sqlite::SQLiteBackend::new()),
            &mut conn1,
        );

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}

#[cfg(all(feature = "pg", feature = "r2d2"))]
#[test]
fn r2d2_pq() {
    let (connspec, _data) = common::pg_connspec();
    let manager = db::ConnectionManager::new(connspec);
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        common::setup_db(Box::new(butane::db::pg::PgBackend::new()), &mut conn1);

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}
