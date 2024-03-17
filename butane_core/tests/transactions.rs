use butane_core::db::{BackendConnection, Connection};
use butane_test_helper::*;

async fn commit_empty_transaction(mut conn: Connection) {
    assert!(!conn.is_closed());

    let tr = conn.transaction().await.unwrap();

    assert!(tr.commit().await.is_ok());
    // it is impossible to reuse the transaction after this.
    // i.e. already_consumed is unreachable.
}
testall_no_migrate!(commit_empty_transaction);

async fn rollback_empty_transaction(mut conn: Connection) {
    let tr = conn.transaction().await.unwrap();

    assert!(tr.rollback().await.is_ok());
    // it is impossible to reuse the transaction after this.
    // i.e. already_consumed is unreachable.
}
testall_no_migrate!(rollback_empty_transaction);

async fn debug_transaction_before_consuming(mut conn: Connection) {
    let backend_name = conn.backend_name();

    let tr = conn.transaction().await.unwrap();

    if backend_name == "pg" {
        assert!(format!("{:?}", tr).contains("{ trans: true }"));
    } else {
        assert!(format!("{:?}", tr).contains("path: Some(\"\")"));
    }

    assert!(tr.commit().await.is_ok());
}
testall_no_migrate!(debug_transaction_before_consuming);
