use butane_core::db::ConnectionAsync;
use butane_test_helper::*;
use butane_test_macros::butane_test;

#[butane_test(nomigrate)]
async fn commit_empty_transaction(mut conn: ConnectionAsync) {
    assert!(!conn.is_closed());

    let tr = conn.transaction().await.unwrap();

    assert!(tr.commit().await.is_ok());
    // it is impossible to reuse the transaction after this.
    // i.e. already_consumed is unreachable.
}

#[butane_test(nomigrate)]
async fn rollback_empty_transaction(mut conn: ConnectionAsync) {
    let tr = conn.transaction().await.unwrap();

    assert!(tr.rollback().await.is_ok());
    // it is impossible to reuse the transaction after this.
    // i.e. already_consumed is unreachable.
}

#[butane_test(nomigrate)]
async fn debug_transaction_before_consuming(mut conn: ConnectionAsync) {
    let backend_name = conn.backend_name();

    let tr = conn.transaction().await.unwrap();

    if backend_name == "pg" {
        assert!(format!("{:?}", tr).contains("{ trans: true }"));
    } else {
        assert!(format!("{:?}", tr).contains("path: Some(\"\")"));
    }

    assert!(tr.commit().await.is_ok());
}
