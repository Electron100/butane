//! PostgreSQL test server management.
//!
//! This module provides functionality to create temporary PostgreSQL servers for testing.
//! It supports two backends:
//! - ephemeralpg's `pg_tmp` command (preferred if available)
//! - Manual server creation using `initdb` and `postgres` (fallback)

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use std::sync::Mutex;

use block_id::{Alphabet, BlockId};

// Global mutex to serialize pg_tmp calls to avoid race conditions
// ephemeralpg's pg_tmp has internal state that can conflict when called concurrently
static PG_TMP_LOCK: Mutex<()> = Mutex::new(());

/// Options for creating a PostgreSQL server.
#[derive(Clone, Debug, Default)]
pub struct PgServerOptions {
    /// The port to listen on. If None, only allow connections via unix sockets.
    pub port: Option<u16>,
    /// The user to connect as. If None, use the default user.
    pub user: Option<String>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    /// Use abstract namespace for the socket.
    ///
    /// Postgres only supports this on Linux and Windows.
    /// However rust-postgres does not yet support it.
    /// <https://github.com/sfackler/rust-postgres/issues/1240>
    pub abstract_namespace: bool,
    /// Callback to run at exit.
    pub atexit_callback: Option<extern "C" fn()>,
    /// Wait time in seconds before automatic cleanup (for ephemeralpg).
    /// If None, uses pg_tmp's default (60 seconds).
    pub ephemeralpg_wait_seconds: Option<u32>,
}

/// Server state for a test PostgreSQL server.
#[derive(Debug)]
pub struct PgServerState {
    /// Temporary directory containing the test server (not used for ephemeralpg)
    pub dir: PathBuf,
    /// Directory for the socket (not used for ephemeralpg)
    pub sockdir: tempfile::TempDir,
    /// Process of the test server
    pub proc: std::process::Child,
    /// stderr from the test server
    pub stderr: BufReader<ChildStderr>,
    /// Options used to create the server.
    pub options: PgServerOptions,
    /// Connection URI from pg_tmp (only set when using ephemeralpg)
    pub ephemeralpg_uri: Option<String>,
}

/// Clean up shared memory segments associated with a PostgreSQL data directory.
/// This is needed on macOS where PostgreSQL can leave orphaned segments.
///
/// Returns true if cleanup was attempted and succeeded, false if no cleanup was needed
/// or if cleanup failed.
#[cfg(target_os = "macos")]
pub fn cleanup_postgres_shared_memory(data_dir: &std::path::Path) -> bool {
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

impl Drop for PgServerState {
    fn drop(&mut self) {
        // Avoid using Child.kill on Unix, as it uses SIGKILL, which postgresql recommends against,
        // and is known to cause shared memory leakage on macOS.
        // See Notes section of https://www.postgresql.org/docs/current/app-postgres.html
        #[cfg(windows)]
        self.proc.kill().ok();
        #[cfg(not(windows))]
        unsafe {
            libc::kill(self.proc.id() as i32, libc::SIGTERM);
        }

        // Wait for the process to exit
        let mut buf = String::new();
        self.stderr.read_to_string(&mut buf).unwrap();
        if !buf.is_empty() {
            log::warn!("pg shutdown error: {buf}");
        }

        // Clean up shared memory segments (macOS-specific issue)
        #[cfg(target_os = "macos")]
        if !self.dir.as_os_str().is_empty() {
            cleanup_postgres_shared_memory(&self.dir);
        }

        // Only delete directory for custom postgres, not for ephemeralpg
        if self.ephemeralpg_uri.is_none() && !self.dir.as_os_str().is_empty() {
            log::info!("Deleting {}", self.dir.display());
            std::fs::remove_dir_all(&self.dir).unwrap();
        }
    }
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
fn is_pg_tmp_available() -> bool {
    which::which("pg_tmp").is_ok()
}

/// Check if initdb is available in PATH.
fn is_initdb_available() -> bool {
    which::which("initdb").is_ok()
}

/// Create a temporary PostgreSQL server using ephemeralpg's pg_tmp command.
///
/// This function is thread-safe - it uses a global mutex to serialize calls to pg_tmp
/// to avoid race conditions in ephemeralpg's internal state management.
pub fn pg_tmp_server_create_ephemeralpg(
    options: PgServerOptions,
) -> Result<PgServerState, PgTemporaryServerError> {
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
    let dir = PathBuf::from("");
    let sockdir = tempfile::TempDir::new()?;
    let stderr = BufReader::new(proc.stderr.take().unwrap());

    if let Some(cb) = options.atexit_callback {
        log::info!("registering atexit callback");
        unsafe {
            libc::atexit(cb);
        }
    }

    Ok(PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
        options: options.clone(),
        ephemeralpg_uri: Some(uri.to_string()),
    })
}

/// Create and start a temporary PostgreSQL server instance.
///
/// This function automatically detects and prefers `pg_tmp` (ephemeralpg) if available,
/// otherwise falls back to using `initdb` and `postgres` directly.
///
/// Fails on Windows CI due to:
/// > The server must be started under an unprivileged user ID to prevent
/// > possible system security compromises. ...
pub fn pg_tmp_server_create(
    options: PgServerOptions,
) -> Result<PgServerState, PgTemporaryServerError> {
    // Try pg_tmp first if available
    if is_pg_tmp_available() {
        log::debug!("pg_tmp detected, using ephemeralpg");
        pg_tmp_server_create_ephemeralpg(options)
    } else if is_initdb_available() {
        log::debug!("pg_tmp not found, falling back to initdb");
        pg_tmp_server_create_using_initdb(options)
    } else {
        Err(PgTemporaryServerError::EphemeralPgError(
            "Neither pg_tmp nor initdb found in PATH. Please install ephemeralpg or PostgreSQL."
                .to_string(),
        ))
    }
}

/// Create and start a temporary PostgreSQL server instance using initdb.
pub fn pg_tmp_server_create_using_initdb(
    options: PgServerOptions,
) -> Result<PgServerState, PgTemporaryServerError> {
    // Otherwise use the custom postgres implementation
    let seed: u128 = rand::random::<u64>() as u128;
    let instance_id = BlockId::new(Alphabet::alphanumeric(), seed, 8)
        .encode_string(0)
        .unwrap();
    // create a temporary directory
    let dir = std::env::current_dir()
        .unwrap()
        .join("tmp_pg")
        .join(instance_id);
    std::fs::create_dir_all(&dir).unwrap();

    let user = options
        .user
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    // Run initdb to create a postgres cluster in our temporary director
    let result = Command::new("initdb")
        .arg("-D")
        .arg(&dir)
        .arg("-U")
        .arg(user)
        .output();
    if let Err(e) = result {
        eprintln!("failed to run initdb; PostgreSQL may not be installed");
        return Err(e.into());
    }

    let output = result.unwrap();

    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("PostgreSQL initdb failed")
    }

    let sockdir = tempfile::TempDir::new().unwrap();

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let socket_directory_arg = if options.abstract_namespace {
        // Use abstract namespace for the socket
        format!("@{}", sockdir.path().display())
    } else {
        // Use a normal socket
        sockdir.path().display().to_string()
    };
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let socket_directory_arg = sockdir.path().display().to_string();

    // Run postgres to actually create the server
    // See https://www.postgresql.org/docs/current/app-postgres.html for CLI args.
    // PGOPTIONS can be used to set args.
    // PGDATA can be used instead of -D
    let mut command = Command::new("postgres");
    command
        .arg("-c")
        .arg("logging_collector=false")
        .arg("-D")
        .arg(&dir)
        .arg("-k")
        .arg(socket_directory_arg)
        .stderr(Stdio::piped());

    if let Some(port) = options.port {
        command
            .arg("-i")
            .arg("-h")
            .arg("localhost")
            .arg("-p")
            .arg(port.to_string());
    } else {
        // Set host='' to prevent postgres from trying to use TCP/IP
        command.arg("-h").arg("");
    }
    let result = command.spawn();
    if let Err(e) = result {
        eprintln!("failed to run postgres");
        return Err(e.into());
    }

    let mut proc = result.unwrap();

    let mut buf = String::new();
    let mut stderr = BufReader::new(proc.stderr.take().unwrap());
    loop {
        buf.clear();
        stderr.read_line(&mut buf).unwrap();
        log::trace!("{buf}");
        if buf.contains("ready to accept connections") {
            break;
        }
        if proc.try_wait().unwrap().is_some() {
            buf.clear();
            stderr.read_to_string(&mut buf).unwrap();
            log::error!("{buf}");
            panic!("postgres process died");
        }
    }
    log::info!("created tmp pg server {}", sockdir.path().display());
    if let Some(cb) = options.atexit_callback {
        // Register the callback to be called when the process exits
        // This is not safe, but this is in a test context and the process will exit.
        // Use `atexit` to ensure that the callback is called even if
        // the process exits unexpectedly.
        log::info!("registering atexit callback");
        unsafe {
            libc::atexit(cb);
        }
    }

    Ok(PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
        options: options.clone(),
        ephemeralpg_uri: None,
    })
}
