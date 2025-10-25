//! Tests for shared memory cleanup functionality.
//!
//! These tests verify that the library properly cleans up PostgreSQL shared memory
//! segments on macOS, where orphaned segments can accumulate and cause "No space left
//! on device" errors.
//!
//! These tests are only compiled and run on macOS where the shared memory issue exists.

#![cfg(target_os = "macos")]

use std::process::Command;

use butane_test_helper::{
    cleanup_postgres_shared_memory, pg_tmp_server_create_using_initdb, PgServerOptions,
};

/// Check if initdb is available in PATH
fn is_initdb_available() -> bool {
    which::which("initdb").is_ok()
}

/// Get count of shared memory segments owned by current user
fn count_shared_memory_segments() -> usize {
    let output = Command::new("ipcs")
        .arg("-m")
        .output()
        .expect("Failed to run ipcs");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let username = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

    output_str
        .lines()
        .filter(|line| line.contains(&username))
        .count()
}

/// Test that shared memory cleanup is called and works correctly
#[test]
fn cleanup_on_drop() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    // Count initial shared memory segments
    let initial_count = count_shared_memory_segments();
    println!("Initial shared memory segments: {}", initial_count);

    // Create a PostgreSQL server
    let options = PgServerOptions::default();
    let server =
        pg_tmp_server_create_using_initdb(options).expect("Failed to create PostgreSQL server");

    println!("Created PostgreSQL server at: {}", server.dir.display());

    // While server is running, count shared memory segments
    let during_count = count_shared_memory_segments();
    println!("Shared memory segments while running: {}", during_count);

    // We expect at least one more segment while the server is running
    assert!(
        during_count > initial_count,
        "Expected more than {} segments while server running, found {}",
        initial_count,
        during_count
    );

    // Drop the server (triggers cleanup)
    let data_dir = server.dir.clone();
    drop(server);

    println!("Server dropped, checking cleanup...");

    // Wait a moment for cleanup to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Count final shared memory segments
    let final_count = count_shared_memory_segments();
    println!("Final shared memory segments: {}", final_count);

    // The count should be less than during (cleanup happened)
    // We're more lenient about the final count vs initial because:
    // 1. Other tests might leave segments
    // 2. The important thing is that THIS server's segment was cleaned up
    assert!(
        final_count < during_count,
        "Expected cleanup to reduce shared memory. During: {}, Final: {}",
        during_count,
        final_count
    );

    // Clean up the data directory if it still exists
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }
}

/// Test the cleanup function directly
#[test]
fn cleanup_function_with_running_server() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    // Create a PostgreSQL server
    let options = PgServerOptions::default();
    let server =
        pg_tmp_server_create_using_initdb(options).expect("Failed to create PostgreSQL server");

    let data_dir = server.dir.clone();
    println!("Created PostgreSQL server at: {}", data_dir.display());

    // Verify postmaster.pid exists
    let postmaster_pid = data_dir.join("postmaster.pid");
    assert!(
        postmaster_pid.exists(),
        "postmaster.pid should exist at {}",
        postmaster_pid.display()
    );

    // Read the postmaster.pid to verify it has a shared memory key
    let content = std::fs::read_to_string(&postmaster_pid).expect("Failed to read postmaster.pid");
    let lines: Vec<&str> = content.lines().collect();

    println!("postmaster.pid has {} lines", lines.len());
    if lines.len() > 5 {
        println!("Shared memory key line: '{}'", lines[5]);
    }

    // We should have a shared memory key on line 6 (index 5)
    assert!(
        lines.len() > 5,
        "postmaster.pid should have at least 6 lines"
    );

    // Explicitly clean up before drop
    let initial_count = count_shared_memory_segments();
    println!("Segments before manual cleanup: {}", initial_count);

    // Try manual cleanup while server is still running
    // This should find and clean up the shared memory
    let cleaned = cleanup_postgres_shared_memory(&data_dir);
    println!("Manual cleanup result: {}", cleaned);

    let after_cleanup_count = count_shared_memory_segments();
    println!("Segments after manual cleanup: {}", after_cleanup_count);

    // If cleanup returned true, we should see a reduction
    if cleaned {
        assert!(
            after_cleanup_count < initial_count,
            "Expected reduction in shared memory segments after cleanup"
        );
    }

    // Now drop the server
    drop(server);

    // Clean up the data directory
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }
}

/// Test that cleanup handles non-existent directories gracefully
#[test]
fn nonexistent_directory() {
    let fake_dir = std::path::PathBuf::from("/tmp/nonexistent_postgres_dir_12345");
    let result = cleanup_postgres_shared_memory(&fake_dir);

    // Should return false (no cleanup needed/possible)
    assert!(
        !result,
        "Cleanup of nonexistent directory should return false"
    );
}

/// Test that cleanup handles directory without postmaster.pid gracefully
#[test]
fn directory_without_postmaster_pid() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let result = cleanup_postgres_shared_memory(temp_dir.path());

    // Should return false (no postmaster.pid to read)
    assert!(
        !result,
        "Cleanup of directory without postmaster.pid should return false"
    );
}

/// Test multiple servers to ensure cleanup works correctly with multiple instances
#[test]
fn multiple_servers() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    let initial_count = count_shared_memory_segments();
    println!("Initial shared memory segments: {}", initial_count);

    // Create multiple servers
    let mut servers = Vec::new();
    for i in 0..3 {
        let options = PgServerOptions::default();
        match pg_tmp_server_create_using_initdb(options) {
            Ok(server) => {
                println!("Created server {} at: {}", i, server.dir.display());
                servers.push(server);
            }
            Err(e) => {
                println!("Failed to create server {}: {:?}", i, e);
                // On macOS with limited shared memory, this might fail
                // That's okay - we'll test with what we have
                break;
            }
        }
    }

    let during_count = count_shared_memory_segments();
    println!(
        "Shared memory segments with {} servers: {}",
        servers.len(),
        during_count
    );

    // Only test cleanup if we actually created servers
    if servers.is_empty() {
        println!("No servers created, skipping cleanup test");
        return;
    }

    // Drop all servers
    servers.clear();

    // Wait for cleanup
    std::thread::sleep(std::time::Duration::from_millis(500));

    let final_count = count_shared_memory_segments();
    println!("Final shared memory segments: {}", final_count);

    // Verify cleanup happened (count decreased)
    assert!(
        final_count < during_count,
        "Expected cleanup to reduce shared memory. During: {}, Final: {}",
        during_count,
        final_count
    );
}
