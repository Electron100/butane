//! Integration tests for libsql-server (sqld) support
//!
//! These tests verify that the sqld server can be started, managed, and used
//! for executing SQL queries in a test environment.

#![cfg(feature = "libsql")]

use butane_test_helper::{libsql_connspec, libsql_setup, libsql_teardown, SetupData};

/// Test sqld server setup and teardown
#[tokio::test]
async fn server_setup() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");
    assert!(setup_data.connection_string().starts_with("libsql+http://"));
    assert!(setup_data.server().port > 0);

    // Test that we can create a connection spec from the server
    let connspec = libsql_connspec(setup_data.server());
    assert!(connspec.connection_string().starts_with("libsql+http://"));
    assert_eq!(connspec.backend_name(), "libsql");

    libsql_teardown(setup_data).await;
}

/// Test that we can create multiple servers
#[tokio::test]
async fn multiple_servers() {
    let setup_data1 = libsql_setup().await.expect("Failed to setup first server");
    let setup_data2 = libsql_setup().await.expect("Failed to setup second server");

    // Servers should be on different ports
    assert_ne!(setup_data1.server().port, setup_data2.server().port);

    libsql_teardown(setup_data1).await;
    libsql_teardown(setup_data2).await;
}

/// Test that server URL format is correct
#[tokio::test]
async fn server_url_format() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");

    let url = setup_data.server().connection_url();
    assert!(url.starts_with("libsql+http://"));
    assert!(url.contains("127.0.0.1:"));

    libsql_teardown(setup_data).await;
}

/// Test that connection spec can be created from server
#[tokio::test]
async fn connspec_from_server() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");

    let connspec = libsql_connspec(setup_data.server());

    assert_eq!(connspec.backend_name(), "libsql");
    assert!(connspec.connection_string().starts_with("libsql+http://"));
    assert!(connspec
        .connection_string()
        .contains(&setup_data.server().port.to_string()));

    libsql_teardown(setup_data).await;
}

/// Test that we can execute a simple SQL query on the server
///
/// This test uses the libsql crate directly rather than the butane backend
/// to verify that the sqld server is functioning correctly at the protocol level.
#[tokio::test]
async fn simple_sql_query() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");

    // Connect directly using libsql crate with HTTP URL
    let http_url = format!("http://127.0.0.1:{}", setup_data.server().port);
    let db = libsql::Builder::new_remote(http_url, "".to_string())
        .build()
        .await
        .expect("Failed to build database");

    let conn = db.connect().expect("Failed to connect to database");

    // Execute a simple query to create a table
    conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", ())
        .await
        .expect("Failed to create table");

    // Insert a row
    conn.execute("INSERT INTO test (id, name) VALUES (1, 'test')", ())
        .await
        .expect("Failed to insert row");

    // Query the row back
    let mut rows = conn
        .query("SELECT * FROM test WHERE id = 1", ())
        .await
        .expect("Failed to query row");

    let row = rows
        .next()
        .await
        .expect("Failed to get next row")
        .expect("No row returned");
    let id: i64 = row.get(0).expect("Failed to get id");
    let name: String = row.get(1).expect("Failed to get name");

    assert_eq!(id, 1);
    assert_eq!(name, "test");

    libsql_teardown(setup_data).await;
}

/// Test that we can execute a simple SQL query using libsql:// scheme
///
/// This test uses the libsql:// connection scheme directly to verify
/// that the sqld server works with the native libSQL protocol.
#[tokio::test]
#[ignore]
async fn simple_sql_query_with_libsql_scheme() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");

    // Connect using libsql:// scheme
    let libsql_url = format!("{}?tls=0", setup_data.server().connection_url());
    let db = libsql::Builder::new_remote(libsql_url, "".to_string())
        .build()
        .await
        .expect("Failed to build database");

    let conn = db.connect().expect("Failed to connect to database");

    // Execute a simple query to create a table
    conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", ())
        .await
        .expect("Failed to create table");

    // Insert a row
    conn.execute("INSERT INTO test (id, name) VALUES (1, 'test')", ())
        .await
        .expect("Failed to insert row");

    // Query the row back
    let mut rows = conn
        .query("SELECT * FROM test WHERE id = 1", ())
        .await
        .expect("Failed to query row");

    let row = rows
        .next()
        .await
        .expect("Failed to get next row")
        .expect("No row returned");
    let id: i64 = row.get(0).expect("Failed to get id");
    let name: String = row.get(1).expect("Failed to get name");

    assert_eq!(id, 1);
    assert_eq!(name, "test");

    libsql_teardown(setup_data).await;
}

/// Test that we can execute a simple SQL query without a URL scheme
///
/// This test uses just the host:port to verify if libsql can connect
/// without an explicit scheme.
#[tokio::test]
#[ignore]
async fn simple_sql_query_without_scheme() {
    let setup_data = libsql_setup().await.expect("Failed to setup server");

    // Connect using just host:port without scheme
    let url = format!("127.0.0.1:{}", setup_data.server().port);
    let db = libsql::Builder::new_remote(url, "".to_string())
        .build()
        .await
        .expect("Failed to build database");

    let conn = db.connect().expect("Failed to connect to database");

    // Execute a simple query to create a table
    conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", ())
        .await
        .expect("Failed to create table");

    // Insert a row
    conn.execute("INSERT INTO test (id, name) VALUES (1, 'test')", ())
        .await
        .expect("Failed to insert row");

    // Query the row back
    let mut rows = conn
        .query("SELECT * FROM test WHERE id = 1", ())
        .await
        .expect("Failed to query row");

    let row = rows
        .next()
        .await
        .expect("Failed to get next row")
        .expect("No row returned");
    let id: i64 = row.get(0).expect("Failed to get id");
    let name: String = row.get(1).expect("Failed to get name");

    assert_eq!(id, 1);
    assert_eq!(name, "test");

    libsql_teardown(setup_data).await;
}
