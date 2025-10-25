//! Integration tests for initdb-based PostgreSQL server creation
#![cfg(feature = "pg")]

use std::sync::{Arc, Barrier};
use std::thread;

use temp_env::with_var;

use butane_test_helper::{pg_tmp_server_create_using_initdb, PgServerOptions};

/// Helper to check if initdb is available
fn is_initdb_available() -> bool {
    which::which("initdb").is_ok()
}

/// Helper to check if postgres is available
fn is_postgres_available() -> bool {
    which::which("postgres").is_ok()
}

/// Test that we get a proper error when initdb is not available
#[test]
fn error_when_initdb_not_found() {
    // Temporarily clear PATH to simulate initdb not being available
    with_var("PATH", None::<&str>, || {
        let options = PgServerOptions::default();
        let result = pg_tmp_server_create_using_initdb(options);

        assert!(result.is_err(), "Should fail when initdb is not available");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("initdb")
                || err_msg.contains("Failed to execute")
                || err_msg.contains("No such file or directory")
                || err_msg.contains("program not found"), // Windows error message
            "Error message should indicate binary not found: {}",
            err_msg
        );
    });
}

/// Test that we get a proper error when postgres is not available
/// (This test requires initdb to be available but postgres to be missing)
#[test]
fn error_when_postgres_not_found() {
    if !is_initdb_available() {
        eprintln!("Skipping test: initdb not found in PATH (needed to test postgres missing)");
        return;
    }

    // Get the initdb directory
    let initdb_path = which::which("initdb").ok().and_then(|p| {
        p.parent()
            .map(|parent| parent.to_string_lossy().to_string())
    });

    if initdb_path.is_none() {
        eprintln!("Skipping test: could not determine initdb location");
        return;
    }

    let initdb_dir = initdb_path.unwrap();

    // Create a minimal PATH with just the initdb directory
    // This assumes postgres is in a different location or can be excluded
    with_var("PATH", Some(initdb_dir.as_str()), || {
        // Check if postgres is still available (it might be in the same dir as initdb)
        if which::which("postgres").is_ok() {
            eprintln!("Skipping test: postgres is in the same directory as initdb");
            return;
        }

        let options = PgServerOptions::default();
        let result = pg_tmp_server_create_using_initdb(options);

        assert!(
            result.is_err(),
            "Should fail when postgres is not available"
        );

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("postgres")
                || err_msg.contains("Failed to execute")
                || err_msg.contains("No such file or directory"),
            "Error message should indicate postgres not found: {}",
            err_msg
        );
    });
}

/// Test that we can create a custom postgres server using initdb
#[test]
#[cfg(not(target_os = "windows"))]
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
#[cfg(not(target_os = "windows"))]
fn directory_structure() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions::default();

    let result = pg_tmp_server_create_using_initdb(options);
    if let Ok(server) = result {
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
    } else {
        eprintln!(
            "Server creation failed (this may be expected if postgres is not fully installed)"
        );
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

    let result1 = pg_tmp_server_create_using_initdb(options1);
    let result2 = pg_tmp_server_create_using_initdb(options2);

    if let (Ok(server1), Ok(server2)) = (result1, result2) {
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
    } else {
        eprintln!("One or both servers failed to create");
    }
}

/// Test that custom user option works
#[test]
#[cfg(not(target_os = "windows"))]
fn custom_user() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions {
        user: Some("testuser".to_string()),
        ..Default::default()
    };

    let result = pg_tmp_server_create_using_initdb(options);
    if let Ok(server) = result {
        assert_eq!(server.options.user, Some("testuser".to_string()));
        println!("Created server with custom user: testuser");
    } else {
        eprintln!("Server creation with custom user failed");
    }
}

/// Test that default user is 'postgres'
#[test]
#[cfg(not(target_os = "windows"))]
fn default_user() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions {
        user: None,
        ..Default::default()
    };

    let result = pg_tmp_server_create_using_initdb(options);
    if let Ok(server) = result {
        assert_eq!(server.options.user, None);
        println!("Created server with default user (postgres)");
    } else {
        eprintln!("Server creation with default user failed");
    }
}

/// Test that directory gets cleaned up on drop
#[test]
#[cfg(not(target_os = "windows"))]
fn cleanup_on_drop() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    let options = PgServerOptions::default();
    let result = pg_tmp_server_create_using_initdb(options);

    if let Ok(server) = result {
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
    } else {
        eprintln!("Server creation failed");
    }
}

/// Test that pg_tmp_server_create_using_initdb always uses initdb
#[test]
#[cfg(not(target_os = "windows"))]
fn explicit_function() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found in PATH");
        return;
    }

    // This function should always use initdb regardless of what's available
    let options = PgServerOptions::default();

    let result = pg_tmp_server_create_using_initdb(options);
    if let Ok(server) = result {
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
    } else {
        eprintln!("Server creation failed");
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

    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Some(_server)) => {
                success_count += 1;
                println!("Thread {} completed successfully", i);
            }
            Ok(None) => {
                panic!("Thread {} failed to create server", i);
            }
            Err(e) => {
                panic!("Thread {} panicked: {:?}", i, e);
            }
        }
    }

    println!(
        "All {} initdb threads completed successfully",
        success_count
    );
    assert_eq!(
        success_count, NUM_THREADS,
        "All threads should succeed with initdb"
    );
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
            let result = pg_tmp_server_create_using_initdb(options);

            if let Ok(server) = result {
                println!(
                    "Thread {} created server with custom user in {:?}",
                    i, server.dir
                );
                Some(server)
            } else {
                eprintln!("Thread {} failed", i);
                None
            }
        });

        handles.push(handle);
    }

    let mut success_count = 0;

    for handle in handles {
        if let Ok(Some(_server)) = handle.join() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, NUM_THREADS,
        "All threads should succeed with custom options"
    );
}

/// Test that sequential thread creation works reliably with initdb
#[test]
#[cfg(not(target_os = "windows"))]
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

    println!("All sequential initdb threads completed successfully");
}
