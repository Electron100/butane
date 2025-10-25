//! PostgreSQL test server management.
//!
//! This module provides functionality to create temporary PostgreSQL servers for testing.
//! It supports two backends:
//! - ephemeralpg's `pg_tmp` command (preferred if available)
//! - Manual server creation using `initdb` and `postgres` (fallback)

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Mutex;

//use block_id::{Alphabet, BlockId};

// Global mutex to serialize pg_tmp calls to avoid race conditions
// ephemeralpg's pg_tmp has internal state that can conflict when called concurrently
static PG_TMP_LOCK: Mutex<()> = Mutex::new(());

/// Clean up shared memory segments associated with a PostgreSQL data directory.
/// This is needed on macOS where PostgreSQL can leave orphaned segments.
///
/// Returns true if cleanup was attempted and succeeded, false if no cleanup was needed
/// or if cleanup failed.
#[cfg(target_os = "macos")]
pub fn cleanup_macos_postgres_shared_memory(data_dir: &std::path::Path) -> bool {
    // PostgreSQL stores shared memory keys in a file in the data directory
    let postmaster_pid_file = data_dir.join("postmaster.pid");
    if !postmaster_pid_file.exists() {
        // File doesn't exist - either not created yet or already cleaned up
        log::debug!("postmaster.pid not found, skipping shared memory cleanup");
        return false;
    }

    // Read the postmaster.pid file to get shared memory keys
    let content = match std::fs::read_to_string(&postmaster_pid_file) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to read postmaster.pid: {}", e);
            return false;
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    // Line 6 (index 5) contains the shared memory key if present
    if lines.len() <= 5 {
        log::debug!("postmaster.pid has no shared memory key entry");
        return false;
    }

    let shmem_key = lines[5].trim();
    if shmem_key.is_empty() || shmem_key == "0" {
        log::debug!("No shared memory key to clean up");
        return false;
    }

    log::info!("Cleaning up shared memory for key: {}", shmem_key);

    // Use ipcs to find the segment ID for this key
    let output = match Command::new("ipcs").arg("-m").output() {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Failed to run ipcs: {}", e);
            return false;
        }
    };

    let output_str = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Failed to parse ipcs output: {}", e);
            return false;
        }
    };

    let mut cleaned = false;
    for line in output_str.lines() {
        if line.contains(shmem_key) {
            // Parse the segment ID from the line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let segment_id = parts[1];
                log::info!("Removing shared memory segment: {}", segment_id);
                match Command::new("ipcrm").arg("-m").arg(segment_id).output() {
                    Ok(output) if output.status.success() => {
                        log::info!("Successfully removed shared memory segment {}", segment_id);
                        cleaned = true;
                    }
                    Ok(output) => {
                        log::warn!(
                            "ipcrm failed with status {}: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    Err(e) => {
                        log::warn!("Failed to run ipcrm: {}", e);
                    }
                }
            }
        }
    }

    cleaned
}

/// Error related to the temporary PostgreSQL server.
#[derive(Debug, thiserror::Error)]
pub enum PgTemporaryServerError {
    /// IO errors.
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    /// Error parsing pg_tmp output.
    #[error("Failed to parse pg_tmp output: {0}")]
    ParseError(String),
    /// pg_tmp command failed.
    #[error("pg_tmp command failed: {0}")]
    EphemeralPgError(String),
}

/// Check if pg_tmp (ephemeralpg) is available in PATH.
pub fn is_pg_tmp_available() -> bool {
    which::which("pg_tmp").is_ok()
}

/// Check if initdb is available in PATH.
pub fn is_initdb_available() -> bool {
    which::which("initdb").is_ok()
}

/// Create a temporary PostgreSQL server using ephemeralpg's pg_tmp command.
///
/// This function is thread-safe - it uses a global mutex to serialize calls to pg_tmp
/// to avoid race conditions in ephemeralpg's internal state management.
pub fn pg_tmp_server_create_ephemeralpg(
    options: crate::PgServerOptions,
) -> Result<crate::PgServerState, PgTemporaryServerError> {
    // Acquire lock to serialize pg_tmp calls across threads
    let _lock = PG_TMP_LOCK.lock().unwrap();

    let mut command = Command::new("pg_tmp");

    // Add wait time option if specified
    if let Some(wait_seconds) = options.ephemeralpg_wait_seconds {
        command.arg("-w").arg(wait_seconds.to_string());
    }

    // Add custom postgres options if specified
    if let Some(port) = options.port {
        command.arg("-o").arg(format!("-p {}", port));
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    log::info!("spawning pg_tmp...");
    let mut proc = command.spawn().map_err(|e| {
        PgTemporaryServerError::EphemeralPgError(format!(
            "Failed to spawn pg_tmp (is ephemeralpg installed?): {}",
            e
        ))
    })?;

    // Read the connection URI from stdout
    let mut stdout = BufReader::new(proc.stdout.take().unwrap());
    let mut uri = String::new();
    stdout.read_line(&mut uri).map_err(|e| {
        PgTemporaryServerError::EphemeralPgError(format!("Failed to read pg_tmp output: {}", e))
    })?;

    let uri = uri.trim();
    if uri.is_empty() {
        // Check if process died
        if let Some(status) = proc.try_wait().map_err(PgTemporaryServerError::IoError)? {
            let mut stderr = BufReader::new(proc.stderr.take().unwrap());
            let mut error_msg = String::new();
            stderr.read_to_string(&mut error_msg).ok();
            return Err(PgTemporaryServerError::EphemeralPgError(format!(
                "pg_tmp exited with status {}: {}",
                status, error_msg
            )));
        }
        return Err(PgTemporaryServerError::ParseError(
            "pg_tmp returned empty URI".to_string(),
        ));
    }

    log::info!("pg_tmp created database: {}", uri);

    // Create dummy paths for compatibility with the struct
    // (these aren't used when using ephemeralpg)
    let dir = std::path::PathBuf::from("");
    let sockdir = tempfile::TempDir::new()?;
    let stderr = BufReader::new(proc.stderr.take().unwrap());

    if let Some(cb) = options.atexit_callback {
        log::info!("registering atexit callback");
        unsafe {
            libc::atexit(cb);
        }
    }

    Ok(crate::PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
        options: options.clone(),
        ephemeralpg_uri: Some(uri.to_string()),
    })
}
