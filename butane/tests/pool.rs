#[cfg(any(feature = "pg", feature = "sqlite"))]
use butane::db::ConnectionManager;
use butane_test_helper::*;
use butane_test_macros::butane_test;
#[cfg(any(feature = "pg", feature = "sqlite"))]
#[cfg(any(feature = "pg", feature = "sqlite"))]
use r2d2;
use std::ops::DerefMut;

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

#[tokio::test]
async fn deadpool_test_pg_async() {
    let (connspec, _data) = pg_connspec().await;
    let manager = ConnectionManager::new(connspec);
    let pool = deadpool::managed::Pool::builder(manager).build().unwrap();
    assert_eq!(pool.status().size, 0);
    assert_eq!(pool.status().available, 0);
    {
        let mut conn: deadpool::managed::Object<ConnectionManager> = pool.get().await.unwrap();
        assert_eq!(pool.status().size, 1);
        assert_eq!(pool.status().available, 0);

        setup_db_async(conn.deref_mut()).await;
    }
    assert_eq!(pool.status().size, 1);
    assert_eq!(pool.status().available, 1);
}
