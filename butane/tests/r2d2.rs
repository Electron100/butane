#[cfg(all(feature = "r2d2", any(feature = "pg", feature = "sqlite")))]
use butane::db::r2::ConnectionManager;
#[cfg(feature = "pg")]
use butane_test_helper::pg_connspec;
#[cfg(any(feature = "pg", feature = "sqlite"))]
use butane_test_helper::setup_db;
#[cfg(feature = "sqlite")]
use butane_test_helper::sqlite_connspec;
#[cfg(all(feature = "r2d2", any(feature = "pg", feature = "sqlite")))]
use r2d2;

#[cfg(feature = "sqlite")]
#[test]
fn r2d2_sqlite() {
    let manager = ConnectionManager::new(sqlite_connspec());
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        setup_db(&mut conn1);

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}

#[cfg(feature = "pg")]
#[test]
fn r2d2_pq() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (connspec, _data) = rt.block_on(pg_connspec());
    let manager = ConnectionManager::new(connspec);
    let pool = r2d2::Pool::builder().max_size(3).build(manager).unwrap();

    {
        let mut conn1 = pool.get().unwrap();
        assert_eq!(pool.state().connections, 3);
        assert_eq!(pool.state().idle_connections, 2);
        setup_db(&mut conn1);

        let _conn2 = pool.get().unwrap();
        assert_eq!(pool.state().idle_connections, 1);
    }
    assert_eq!(pool.state().idle_connections, 3);
}
