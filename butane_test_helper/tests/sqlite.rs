//! Integration tests for SQLite support
#![cfg(feature = "sqlite")]

use butane_test_helper::{
    sqlite_connection, sqlite_connspec, sqlite_setup, sqlite_teardown, SetupData,
};

/// Test that we can create a SQLite in-memory connection
#[test]
fn connection_creation() {
    let conn = sqlite_connection();
    println!("Created SQLite in-memory connection");
    drop(conn);
}

/// Test that we can create a SQLite connection spec
#[test]
fn connspec_creation() {
    let connspec = sqlite_connspec();
    assert_eq!(connspec.connection_string(), ":memory:");
    println!(
        "Created SQLite connection spec: {:?}",
        connspec.connection_string()
    );
}

/// Test SQLite setup and teardown
#[test]
fn setup_teardown() {
    // Use pollster to run async in a sync test
    let setup_data = pollster::block_on(sqlite_setup());
    assert_eq!(setup_data.connection_string(), ":memory:");
    println!("SQLite setup completed");

    sqlite_teardown(setup_data);
    println!("SQLite teardown completed");
}

/// Test multiple SQLite connections can coexist
#[test]
fn multiple_connections() {
    let conn1 = sqlite_connection();
    let conn2 = sqlite_connection();
    let conn3 = sqlite_connection();

    println!("Created 3 independent SQLite in-memory connections");

    // Each connection is independent and has its own in-memory database
    drop(conn1);
    drop(conn2);
    drop(conn3);
}

/// Test that connection string is always :memory:
#[test]
fn always_memory() {
    let connspec = sqlite_connspec();
    assert_eq!(connspec.connection_string(), ":memory:");
    assert!(!connspec.connection_string().contains("file:"));
    assert!(!connspec.connection_string().contains(".db"));
}
