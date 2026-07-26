//! Integration tests for initdb-based PostgreSQL server creation
#![cfg(test)]
#![cfg(feature = "pg")]
#![cfg(not(target_os = "windows"))]

use std::sync::{Arc, Barrier};
use std::thread;

use butane_test_helper::{
    is_initdb_available, is_postgres_available, pg_tmp_server_create_using_initdb, PgServerOptions,
    PgServerState, PgTemporaryServerError,
};

/// Unwrap a server-creation result, failing the test with the underlying error.
///
/// Every test here already guards on `is_initdb_available() && is_postgres_available()`, so an
/// `Err` at this point is a genuine failure rather than a missing install. These sites previously
/// logged and fell through, which reported `ok` while asserting nothing and discarded the one
/// piece of information needed to diagnose the run.
#[track_caller]
fn expect_server(
    result: Result<PgServerState, PgTemporaryServerError>,
    what: &str,
) -> PgServerState {
    match result {
        Ok(server) => server,
        Err(e) => panic!("{what} failed: {e}"),
    }
}

/// Render a joined thread's outcome as `Ok(server)` or a human-readable failure string.
///
/// `join` returns the panic payload as `Box<dyn Any>`, whose `Debug` is just `Any { .. }`; the
/// message only comes out by downcasting to the `&str`/`String` that `panic!` actually stored.
fn joined(
    outcome: std::thread::Result<Result<PgServerState, String>>,
) -> Result<PgServerState, String> {
    match outcome {
        Ok(inner) => inner,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("<non-string panic payload>");
            Err(format!("thread panicked: {msg}"))
        }
    }
}

/// Test that we can create a custom postgres server using initdb
#[test]
fn server_creation() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions::default();

    let result = pg_tmp_server_create_using_initdb(options);
    assert!(
        result.is_ok(),
        "Failed to create initdb server: {:?}",
        result.err()
    );

    let server = result.unwrap();
    assert!(
        server.ephemeralpg_uri.is_none(),
        "initdb server should not have ephemeralpg URI"
    );
    assert!(
        !server.dir.as_os_str().is_empty(),
        "initdb server should have a directory"
    );

    println!("Created initdb server at: {}", server.dir.display());

    // Server will be dropped here, which should clean it up
}

/// Test that the directory structure is created correctly
#[test]
fn directory_structure() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions::default();

    let server = expect_server(
        pg_tmp_server_create_using_initdb(options),
        "server creation for directory_structure",
    );
    {
        let dir = &server.dir;
        assert!(dir.exists(), "Server directory should exist");
        assert!(dir.is_dir(), "Server path should be a directory");

        println!("Server directory: {}", dir.display());

        // Check for some expected PostgreSQL files
        assert!(
            dir.join("PG_VERSION").exists(),
            "PG_VERSION file should exist"
        );
        assert!(
            dir.join("postgresql.conf").exists(),
            "postgresql.conf should exist"
        );

        println!("Directory structure verified");
    }
}

/// Test creating multiple servers at the same time
#[test]
fn multiple_servers() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options1 = PgServerOptions::default();
    let options2 = PgServerOptions::default();

    let server1 = expect_server(pg_tmp_server_create_using_initdb(options1), "first server");
    let server2 = expect_server(pg_tmp_server_create_using_initdb(options2), "second server");

    {
        // Each server should have a different directory
        assert_ne!(
            server1.dir, server2.dir,
            "Each server should have a unique directory"
        );

        println!("Server 1: {}", server1.dir.display());
        println!("Server 2: {}", server2.dir.display());

        // Both should exist
        assert!(server1.dir.exists());
        assert!(server2.dir.exists());
    }
}

/// Test that custom user option works
#[test]
fn custom_user() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions {
        user: Some("testuser".to_string()),
        ..Default::default()
    };

    let server = expect_server(
        pg_tmp_server_create_using_initdb(options),
        "server creation with custom user",
    );
    assert_eq!(server.options.user, Some("testuser".to_string()));
    println!("Created server with custom user: testuser");
}

/// Test that default user is 'postgres'
#[test]
fn default_user() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions {
        user: None,
        ..Default::default()
    };

    let server = expect_server(
        pg_tmp_server_create_using_initdb(options),
        "server creation with default user",
    );
    assert_eq!(server.options.user, None);
    println!("Created server with default user (postgres)");
}

/// Test that directory gets cleaned up on drop
#[test]
fn cleanup_on_drop() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions::default();
    let server = expect_server(
        pg_tmp_server_create_using_initdb(options),
        "server creation for cleanup_on_drop",
    );

    {
        let dir_path = server.dir.clone();
        println!("Created server at: {}", dir_path.display());

        // Directory should exist while server is alive
        assert!(dir_path.exists(), "Directory should exist");

        // Drop the server
        drop(server);

        // Give it a moment for cleanup
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Directory should be cleaned up
        assert!(
            !dir_path.exists(),
            "Directory should be cleaned up after drop"
        );
        println!("Directory successfully cleaned up");
    }
}

/// Test that pg_tmp_server_create_using_initdb always uses initdb
#[test]
fn explicit_function() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    // This function should always use initdb regardless of what's available
    let options = PgServerOptions::default();

    let server = expect_server(
        pg_tmp_server_create_using_initdb(options),
        "server creation for explicit_function",
    );
    {
        // Should still be using initdb (no ephemeralpg URI)
        assert!(
            server.ephemeralpg_uri.is_none(),
            "initdb function should not set ephemeralpg URI"
        );
        assert!(
            !server.dir.as_os_str().is_empty(),
            "initdb function should create a directory"
        );
        println!("Confirmed that initdb function always uses initdb regardless of flag");
    }
}

/// Test that multiple threads can create servers with custom options
#[test]
fn multithreaded_with_options() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
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

            println!("Thread {} attempting to create initdb server", i);

            let options = PgServerOptions::default();
            let result = pg_tmp_server_create_using_initdb(options);

            match result {
                Ok(server) => {
                    println!("Thread {} successfully created initdb server", i);

                    // Verify the server is usable
                    assert!(
                        server.ephemeralpg_uri.is_none(),
                        "Thread {}: Should not have ephemeralpg URI",
                        i
                    );
                    assert!(
                        !server.dir.as_os_str().is_empty(),
                        "Thread {}: Server should have a directory",
                        i
                    );
                    println!("Thread {} using initdb in {:?}", i, server.dir);

                    // Return the server so it stays alive until the thread completes
                    Ok(server)
                }
                // Carry the error text out of the thread: `join` cannot return the original
                // error type, and the reason is the only thing that makes a failure actionable.
                Err(e) => Err(format!("thread {i}: {e}")),
            }
        });

        handles.push(handle);
    }

    // Collect every outcome before asserting, so one failure reports all of them rather than
    // aborting on the first. `_servers` stays bound so the clusters live until the assert.
    let mut servers = Vec::new();
    let mut failures = Vec::new();
    for handle in handles {
        match joined(handle.join()) {
            Ok(server) => servers.push(server),
            Err(msg) => failures.push(msg),
        }
    }

    assert!(
        failures.is_empty(),
        "all {} threads should succeed with initdb, but {} failed:\n  {}",
        NUM_THREADS,
        failures.len(),
        failures.join("\n  ")
    );
    assert_eq!(servers.len(), NUM_THREADS);
}

/// Test that threads can create initdb servers with custom options
#[test]
fn multithreaded_initdb_with_options() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    const NUM_THREADS: usize = 2;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let mut handles = vec![];

    for i in 0..NUM_THREADS {
        let barrier_clone = Arc::clone(&barrier);
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let options = PgServerOptions {
                user: Some(format!("testuser{}", i)),
                ..Default::default()
            };

            println!("Thread {} creating initdb server with custom user", i);

            match pg_tmp_server_create_using_initdb(options) {
                Ok(server) => {
                    println!(
                        "Thread {} created server with custom user in {:?}",
                        i, server.dir
                    );
                    Ok(server)
                }
                // Previously this logged a bare "Thread N failed" and the assertion only compared
                // counts, so a real failure surfaced as `left: 1, right: 2` with no cause.
                Err(e) => Err(format!("thread {i} (testuser{i}): {e}")),
            }
        });

        handles.push(handle);
    }

    let mut servers = Vec::new();
    let mut failures = Vec::new();
    for handle in handles {
        match joined(handle.join()) {
            Ok(server) => servers.push(server),
            Err(msg) => failures.push(msg),
        }
    }

    assert!(
        failures.is_empty(),
        "all {} threads should succeed with custom options, but {} failed:\n  {}",
        NUM_THREADS,
        failures.len(),
        failures.join("\n  ")
    );
    assert_eq!(servers.len(), NUM_THREADS);
}

/// Test that sequential thread creation works reliably with initdb
#[test]
fn sequential_thread_creation() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    const NUM_THREADS: usize = 3;

    for i in 0..NUM_THREADS {
        let handle = thread::spawn(move || {
            println!("Sequential thread {} creating initdb server", i);

            let options = PgServerOptions::default();
            let result = pg_tmp_server_create_using_initdb(options);

            match result {
                Ok(server) => {
                    println!("Sequential thread {} succeeded in {:?}", i, server.dir);
                    Ok(server)
                }
                Err(e) => Err(format!("sequential thread {i}: {e}")),
            }
        });

        // Wait for this thread to complete before starting the next
        match joined(handle.join()) {
            Ok(_server) => println!("Sequential thread {} completed", i),
            Err(msg) => panic!("{msg}"),
        }
    }

    println!("All sequential initdb threads completed successfully");
}
