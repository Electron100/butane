//! Integration tests for MySQL support
#![cfg(feature = "mysql")]

use butane_test_helper::{mysql_connspec, mysql_connstr, mysql_setup, mysql_teardown, SetupData};

/// Helper to check if mysqld is available
fn is_mysqld_available() -> bool {
    which::which("mysqld").is_ok()
}

/// Test that we can create a MySQL connection spec with setup
#[tokio::test]
async fn connection_creation() {
    if !is_mysqld_available() {
        eprintln!("Skipping test: mysqld not found in PATH");
        return;
    }

    let (connspec, setup_data) = mysql_connspec().await;
    let conn_str = connspec.connection_string();

    println!("Created MySQL connection spec: {}", conn_str);
    assert!(conn_str.contains("mysql://"));

    mysql_teardown(setup_data);
    println!("MySQL teardown completed");
}

/// Test MySQL setup and teardown
#[tokio::test]
async fn setup_teardown() {
    if !is_mysqld_available() {
        eprintln!("Skipping test: mysqld not found in PATH");
        return;
    }

    let setup_data = mysql_setup().await;
    println!("MySQL setup completed");

    // Verify connection string format
    let conn_str = mysql_connstr(&setup_data);
    assert!(conn_str.starts_with("mysql://"));
    println!("Connection string: {}", conn_str);

    mysql_teardown(setup_data);
    println!("MySQL teardown completed");
}

/// Test that connection string has expected format
#[tokio::test]
async fn connection_string_format() {
    if !is_mysqld_available() {
        eprintln!("Skipping test: mysqld not found in PATH");
        return;
    }

    let setup_data = mysql_setup().await;
    let conn_str = mysql_connstr(&setup_data);

    // Should start with mysql://
    assert!(conn_str.starts_with("mysql://"));

    // Should contain user (root)
    assert!(conn_str.contains("root"));

    // Should contain database name (test)
    assert!(conn_str.contains("test"));

    println!("Connection string format validated: {}", conn_str);

    mysql_teardown(setup_data);
}

/// Test multiple MySQL servers can be created
#[tokio::test]
async fn multiple_servers() {
    if !is_mysqld_available() {
        eprintln!("Skipping test: mysqld not found in PATH");
        return;
    }

    let setup1 = mysql_setup().await;
    let setup2 = mysql_setup().await;

    println!("Created 2 independent MySQL servers");

    let conn1 = mysql_connstr(&setup1);
    let conn2 = mysql_connstr(&setup2);

    // Should have different ports or sockets
    assert_ne!(
        conn1, conn2,
        "Each server should have unique connection details"
    );

    println!("Server 1: {}", conn1);
    println!("Server 2: {}", conn2);

    mysql_teardown(setup1);
    mysql_teardown(setup2);
}

/// Test connection string structure
#[tokio::test]
async fn connection_string_structure() {
    if !is_mysqld_available() {
        eprintln!("Skipping test: mysqld not found in PATH");
        return;
    }

    let setup_data = mysql_setup().await;
    let conn_str = setup_data.connection_string();

    println!("Full connection string: {}", conn_str);

    // Parse URL components
    if conn_str.contains("://") {
        let parts: Vec<&str> = conn_str.split("://").collect();
        assert_eq!(parts[0], "mysql", "Protocol should be mysql");
        println!("Protocol: {}", parts[0]);
    }

    mysql_teardown(setup_data);
}
