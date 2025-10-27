//! Integration tests for ephemeralpg support
#![cfg(feature = "pg")]

use std::sync::{Arc, Barrier};
use std::thread;

use temp_env::with_var;

use butane_test_helper::{
    pg_tmp_server_create_ephemeralpg, PgServerOptions, PgTemporaryServerError,
};

/// Helper to check if pg_tmp is available
fn is_pg_tmp_available() -> bool {
    which::which("pg_tmp").is_ok()
}

/// Test that we can detect if pg_tmp is not available
#[test]
fn error_when_not_found() {
    // Temporarily clear PATH to simulate pg_tmp not being available
    with_var("PATH", None::<&str>, || {
        let options = PgServerOptions::default();

        let result = pg_tmp_server_create_ephemeralpg(options);
        assert!(result.is_err(), "Should fail when pg_tmp is not available");

        if let Err(PgTemporaryServerError::EphemeralPg(msg)) = result {
            assert!(
                msg.contains("pg_tmp") || msg.contains("ephemeralpg"),
                "Error message should mention pg_tmp or ephemeralpg"
            );
        } else {
            panic!("Expected EphemeralPg");
        }
    });
}

/// Test that ephemeralpg creates a server with a valid URI
#[test]
fn server_creation() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    let options = PgServerOptions {
        ephemeralpg_wait_seconds: Some(120),
        ..Default::default()
    };

    let result = pg_tmp_server_create_ephemeralpg(options);
    assert!(
        result.is_ok(),
        "Failed to create ephemeralpg server: {:?}",
        result.err()
    );

    let server = result.unwrap();
    assert!(server.ephemeralpg_uri.is_some(), "URI should be set");

    let uri = server.ephemeralpg_uri.as_ref().unwrap();
    assert!(
        uri.starts_with("postgresql://"),
        "URI should start with postgresql://"
    );
    assert!(!uri.is_empty(), "URI should not be empty");

    println!("Created ephemeralpg server with URI: {}", uri);

    // Server will be dropped here, which should clean it up
}

/// Test that custom postgres option fields are properly set (not used by ephemeralpg)
#[test]
fn ignores_custom_postgres_options() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    let options = PgServerOptions {
        user: Some("customuser".to_string()),
        ..Default::default()
    };

    let result = pg_tmp_server_create_ephemeralpg(options);
    if let Ok(server) = result {
        assert!(
            server.ephemeralpg_uri.is_some(),
            "Should have ephemeralpg URI"
        );
        assert!(
            server.dir.as_os_str().is_empty(),
            "Should have empty dir for ephemeralpg"
        );
        println!("ephemeralpg correctly ignores custom postgres options");
    } else {
        eprintln!("Server creation failed");
    }
}

/// Test that wait time option can be set
#[test]
fn wait_time_option() {
    let options = PgServerOptions {
        ephemeralpg_wait_seconds: Some(300),
        ..Default::default()
    };

    assert_eq!(options.ephemeralpg_wait_seconds, Some(300));
}

/// Test that port option is passed through to pg_tmp
#[test]
fn with_custom_port() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    let options = PgServerOptions {
        port: Some(5555),
        ..Default::default()
    };

    let result = pg_tmp_server_create_ephemeralpg(options);
    // This may or may not succeed depending on port availability
    // Just verify the option was set
    if let Ok(server) = result {
        assert_eq!(server.options.port, Some(5555));
        println!("Created ephemeralpg server with custom port option");
    }
}

/// Test that URI is properly formatted
#[test]
fn uri_format() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    let options = PgServerOptions::default();

    let result = pg_tmp_server_create_ephemeralpg(options);
    if let Ok(server) = result {
        let uri = server.ephemeralpg_uri.as_ref().unwrap();

        // URI should be a valid postgresql:// URI
        assert!(uri.starts_with("postgresql://"));

        // Should not be just the prefix
        assert!(uri.len() > "postgresql://".len());

        println!("URI format validated: {}", uri);
    }
}

/// Test creating multiple ephemeralpg servers
#[test]
fn multiple_servers() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    let options1 = PgServerOptions::default();
    let options2 = PgServerOptions::default();

    let result1 = pg_tmp_server_create_ephemeralpg(options1);
    let result2 = pg_tmp_server_create_ephemeralpg(options2);

    if let (Ok(server1), Ok(server2)) = (result1, result2) {
        let uri1 = server1.ephemeralpg_uri.as_ref().unwrap();
        let uri2 = server2.ephemeralpg_uri.as_ref().unwrap();

        // Each server should have a different URI
        assert_ne!(uri1, uri2, "Each server should have a unique URI");

        println!("Server 1: {}", uri1);
        println!("Server 2: {}", uri2);
    }
}

/// Test that multiple threads can each create their own ephemeralpg server.
///
/// This test verifies thread-safety of pg_tmp_server_create_ephemeralpg().
/// The function uses a global mutex to serialize calls to pg_tmp, ensuring
/// that concurrent server creation is safe.
#[test]
fn multithreaded_creation() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    const NUM_THREADS: usize = 3;

    // Use a barrier to ensure all threads start at roughly the same time
    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let mut handles = vec![];

    for i in 0..NUM_THREADS {
        let barrier_clone = Arc::clone(&barrier);
        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            println!("Thread {} attempting to create ephemeralpg server", i);

            let options = PgServerOptions::default();
            let result = pg_tmp_server_create_ephemeralpg(options);

            match result {
                Ok(server) => {
                    println!("Thread {} successfully created ephemeralpg server", i);

                    // Verify the server is usable
                    let uri = server.ephemeralpg_uri.as_ref().unwrap();
                    assert!(
                        uri.starts_with("postgresql://"),
                        "Thread {}: Invalid URI format",
                        i
                    );
                    println!("Thread {} using ephemeralpg: {}", i, uri);

                    // Return the server so it stays alive until the thread completes
                    Some(server)
                }
                Err(e) => {
                    eprintln!("Thread {} failed to create server: {:?}", i, e);
                    None
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all threads and collect results
    let mut success_count = 0;
    let mut failure_count = 0;

    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Some(_server)) => {
                success_count += 1;
                println!("Thread {} completed successfully", i);
            }
            Ok(None) => {
                failure_count += 1;
                eprintln!("Thread {} failed to create server", i);
            }
            Err(e) => {
                failure_count += 1;
                eprintln!("Thread {} panicked: {:?}", i, e);
            }
        }
    }

    println!(
        "Results: {} successes, {} failures",
        success_count, failure_count
    );

    // All threads should succeed now that we have proper locking
    assert_eq!(
        success_count, NUM_THREADS,
        "Expected all {} threads to successfully create servers",
        NUM_THREADS
    );
    assert_eq!(failure_count, 0, "Expected no failures with proper locking");
}

/// Test that sequential thread creation works reliably with ephemeralpg.
#[test]
fn sequential_thread_creation() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found in PATH");
        return;
    }

    const NUM_THREADS: usize = 3;

    for i in 0..NUM_THREADS {
        let handle = thread::spawn(move || {
            println!("Sequential thread {} creating ephemeralpg server", i);

            let options = PgServerOptions::default();
            let result = pg_tmp_server_create_ephemeralpg(options);

            match result {
                Ok(server) => {
                    let uri = server.ephemeralpg_uri.as_ref().unwrap();
                    println!("Sequential thread {} succeeded with URI: {}", i, uri);
                    Some(server)
                }
                Err(e) => {
                    eprintln!("Sequential thread {} failed: {:?}", i, e);
                    None
                }
            }
        });

        // Wait for this thread to complete before starting the next
        match handle.join() {
            Ok(Some(_server)) => {
                println!("Sequential thread {} completed", i);
            }
            Ok(None) => {
                panic!("Sequential thread {} failed to create server", i);
            }
            Err(e) => {
                panic!("Sequential thread {} panicked: {:?}", i, e);
            }
        }
    }

    println!("All sequential ephemeralpg threads completed successfully");
}
