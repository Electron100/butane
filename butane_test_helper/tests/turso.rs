//! Integration tests for Turso support
#![cfg(feature = "turso")]

use butane_test_helper::{turso_connspec, turso_setup, turso_teardown, SetupData};

/// Test that we can create a Turso connection spec
#[test]
fn connspec_creation() {
    let connspec = turso_connspec();
    assert_eq!(connspec.connection_string(), ":memory:");
    println!(
        "Created Turso connection spec: {:?}",
        connspec.connection_string()
    );
}

/// Test that the backend name is correct
#[test]
fn backend_name() {
    let connspec = turso_connspec();
    assert_eq!(connspec.backend_name(), "turso");
    println!("Turso backend name verified: turso");
}

/// Test Turso setup and teardown
#[test]
fn setup_teardown() {
    // Use pollster to run async in a sync test
    let setup_data = pollster::block_on(turso_setup());
    assert_eq!(setup_data.connection_string(), ":memory:");
    println!("Turso setup completed");

    turso_teardown(setup_data);
    println!("Turso teardown completed");
}

/// Test that connection string is always :memory:
#[test]
fn always_memory() {
    let connspec = turso_connspec();
    assert_eq!(connspec.connection_string(), ":memory:");
    assert!(!connspec.connection_string().contains("file:"));
    assert!(!connspec.connection_string().contains(".db"));
}

/// Test that we can get a Turso backend connection spec multiple times
#[test]
fn multiple_connspecs() {
    let connspec1 = turso_connspec();
    let connspec2 = turso_connspec();
    let connspec3 = turso_connspec();

    assert_eq!(connspec1.connection_string(), ":memory:");
    assert_eq!(connspec2.connection_string(), ":memory:");
    assert_eq!(connspec3.connection_string(), ":memory:");

    println!("Created 3 Turso connection specs");
}

/// Test async setup multiple times
#[test]
fn multiple_setups() {
    // Use pollster to run async in a sync test
    let setup_data1 = pollster::block_on(turso_setup());
    let setup_data2 = pollster::block_on(turso_setup());
    let setup_data3 = pollster::block_on(turso_setup());

    assert_eq!(setup_data1.connection_string(), ":memory:");
    assert_eq!(setup_data2.connection_string(), ":memory:");
    assert_eq!(setup_data3.connection_string(), ":memory:");

    turso_teardown(setup_data1);
    turso_teardown(setup_data2);
    turso_teardown(setup_data3);

    println!("Created and cleaned up 3 Turso test databases");
}
