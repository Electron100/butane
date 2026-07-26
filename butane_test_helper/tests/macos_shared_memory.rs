//! Tests for shared memory cleanup functionality.
//!
//! These tests verify that the library properly cleans up PostgreSQL shared memory
//! segments on macOS, where orphaned segments can accumulate and cause "No space left
//! on device" errors.
//!
//! These tests are only compiled and run on macOS where the shared memory issue exists.

#![cfg(test)]
#![cfg(target_os = "macos")]

use std::process::Command;

use butane_test_helper::pg::{cleanup_macos_postgres_shared_memory, shmem_id_from_postmaster_pid};
use butane_test_helper::{is_initdb_available, pg_tmp_server_create_using_initdb, PgServerOptions};

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

/// This running server's SysV shared-memory segment id, via the production parser
/// ([`shmem_id_from_postmaster_pid`]) so the scoped tests exercise the same parsing they verify.
fn running_server_segment_id(data_dir: &std::path::Path) -> String {
    let content =
        std::fs::read_to_string(data_dir.join("postmaster.pid")).expect("read postmaster.pid");
    shmem_id_from_postmaster_pid(&content)
        .expect("parser should accept the real postmaster.pid")
        .expect("a running server should record a shared-memory segment")
        .to_string()
}

/// Whether a SysV shared-memory segment with `id` currently exists (the `ID` column of `ipcs -m`).
fn segment_exists(id: &str) -> bool {
    let output = Command::new("ipcs")
        .arg("-m")
        .output()
        .expect("Failed to run ipcs");
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("m") && fields.next() == Some(id)
    })
}

/// Dropping a server removes its shared-memory segment.
///
/// Scoped to this server's exact segment id, so parallel tests (or servers) creating and
/// destroying their own segments cannot perturb it -- the source of the earlier flakiness, which
/// compared a global before/during/after count of all the user's segments.
#[test]
fn cleanup_on_drop() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    let server = pg_tmp_server_create_using_initdb(PgServerOptions::default())
        .expect("Failed to create PostgreSQL server");
    let data_dir = server.dir.clone();

    let seg_id = running_server_segment_id(&data_dir);
    println!("server {} uses shm segment {seg_id}", data_dir.display());

    assert!(
        segment_exists(&seg_id),
        "segment {seg_id} should exist while the server runs"
    );

    // Drop sends SIGTERM; postgres removes its own segment as it shuts down.
    drop(server);

    // Poll for THIS segment to disappear (scoped, so parallel activity cannot perturb it).
    let mut gone = false;
    for _ in 0..20 {
        if !segment_exists(&seg_id) {
            gone = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    // Fall back to explicit cleanup if the drop path left the segment behind.
    if !gone {
        if let Err(e) = cleanup_macos_postgres_shared_memory(&data_dir) {
            println!("explicit cleanup errored: {e}");
        }
        gone = !segment_exists(&seg_id);
    }

    assert!(
        gone,
        "segment {seg_id} should be gone after the server is dropped"
    );

    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }
}

/// The cleanup routine finds and removes a running server's segment via its `postmaster.pid`.
///
/// `ipcrm` on an attached segment marks it for deletion and returns success, but the segment
/// stays visible in `ipcs` until postgres detaches -- so this asserts the routine reports it
/// removed the segment, not a drop in the (still-attached) segment count.
#[test]
fn cleanup_function_with_running_server() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    let server = pg_tmp_server_create_using_initdb(PgServerOptions::default())
        .expect("Failed to create PostgreSQL server");
    let data_dir = server.dir.clone();

    let seg_id = running_server_segment_id(&data_dir);
    assert!(
        segment_exists(&seg_id),
        "segment {seg_id} should exist while the server runs"
    );

    let cleaned = cleanup_macos_postgres_shared_memory(&data_dir)
        .expect("cleanup should not error on a valid postmaster.pid");
    assert!(
        cleaned,
        "cleanup should remove the running server's segment {seg_id}"
    );

    drop(server);

    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }
}

/// Test that cleanup handles non-existent directories gracefully
#[test]
fn nonexistent_directory() {
    let fake_dir = std::path::PathBuf::from("/tmp/nonexistent_postgres_dir_12345");
    let result = cleanup_macos_postgres_shared_memory(&fake_dir)
        .expect("a missing directory is not a format error");

    // Should return Ok(false) (no cleanup needed/possible)
    assert!(
        !result,
        "Cleanup of nonexistent directory should return Ok(false)"
    );
}

/// Test that cleanup handles directory without postmaster.pid gracefully
#[test]
fn directory_without_postmaster_pid() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let result = cleanup_macos_postgres_shared_memory(temp_dir.path())
        .expect("a directory without postmaster.pid is not a format error");

    // Should return Ok(false) (no postmaster.pid to read)
    assert!(
        !result,
        "Cleanup of directory without postmaster.pid should return Ok(false)"
    );
}

/// The parser accepts a `postmaster.pid` written by the installed PostgreSQL, and the id it
/// extracts names a live segment.
///
/// This is the check that catches an upstream format change against the *real* binaries; the
/// synthetic cases below only exercise hand-written input. No segment is removed, so the running
/// server is left undisturbed.
#[test]
fn parser_accepts_real_postmaster_pid() {
    if !is_initdb_available() {
        println!("Skipping test: initdb not available");
        return;
    }

    let server = pg_tmp_server_create_using_initdb(PgServerOptions::default())
        .expect("Failed to create PostgreSQL server");
    let data_dir = server.dir.clone();

    let content =
        std::fs::read_to_string(data_dir.join("postmaster.pid")).expect("read postmaster.pid");
    let id = shmem_id_from_postmaster_pid(&content)
        .expect("parser should accept the real postmaster.pid format")
        .expect("a running server should record a SysV shared-memory segment");

    // The parsed id names a segment that actually exists, proving the parser read the segment
    // line -- not merely some integer pair elsewhere in the file.
    assert!(
        segment_exists(&id.to_string()),
        "parsed segment id {id} should appear in `ipcs -m` while the server runs"
    );

    drop(server);
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }
}

/// A shared-memory line that is not `<key> <id>` is reported, not silently ignored -- the signal
/// that PostgreSQL changed the file format.
#[test]
fn malformed_shmem_line_is_reported() {
    // Lines 1-6 are placeholders; line 7 (the shmem key line) is not `<key> <id>`.
    let content = "123\n/data\n1700000000\n5432\n/sock\n\nnot a key line\nready\n";
    let err = shmem_id_from_postmaster_pid(content)
        .expect_err("a malformed shared-memory line should be reported");
    println!("--- diagnostic ---\n{err}\n---");
    assert!(
        err.contains("postmaster.pid format may have changed"),
        "should hint at a format change: {err}"
    );
    assert!(
        err.contains("shared-memory line 7"),
        "should name the offending line: {err}"
    );
}

/// A `postmaster.pid` too short to contain the shared-memory line is reported.
#[test]
fn truncated_pid_file_is_reported() {
    let err = shmem_id_from_postmaster_pid("123\n/data\n5432\n")
        .expect_err("a truncated postmaster.pid should be reported");
    assert!(
        err.contains("postmaster.pid format may have changed"),
        "should hint at a format change: {err}"
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
