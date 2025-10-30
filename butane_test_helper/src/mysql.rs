//! MySQL test server management.
//!
//! This module provides functionality to create temporary MySQL servers for testing.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;
use tempfile::TempDir;
use thiserror::Error;
use which;

/// Ensures cleanup is only run once per test session
static CLEANUP_ONCE: Once = Once::new();

/// Errors that can occur when creating a temporary MySQL server.
#[derive(Debug, Error)]
pub enum MySqlTemporaryServerError {
    /// Failed to create temporary directory.
    #[error("Failed to create temporary directory: {0}")]
    TempDir(#[from] std::io::Error),
    /// MySQL initialization failed.
    #[error("MySQL initialization failed: {0}")]
    InitFailed(String),
    /// MySQL server failed to start.
    #[error("MySQL server failed to start: {0}")]
    ServerStartFailed(String),
    /// MySQL is not installed or not found in PATH.
    #[error("MySQL not found in PATH. Please install MySQL.")]
    MySqlNotFound,
}

/// Data for a temporary MySQL server instance.
#[derive(Debug)]
pub struct MySqlSetupData {
    /// Temporary directory containing MySQL data.
    _data_dir: TempDir,
    /// Port the MySQL server is running on.
    port: u16,
    /// Socket file path (for Unix systems).
    socket: Option<PathBuf>,
    /// MySQL server process handle.
    server_process: Option<std::process::Child>,
}

impl MySqlSetupData {
    /// Get the connection string for this MySQL instance.
    pub fn connection_string(&self) -> String {
        if let Some(socket) = &self.socket {
            format!("mysql://root@localhost/test?socket={}", socket.display())
        } else {
            format!("mysql://root@localhost:{}/test", self.port)
        }
    }

    /// Get the port number.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the socket path if available.
    pub fn socket(&self) -> Option<&PathBuf> {
        self.socket.as_ref()
    }
}

/// Create a temporary MySQL server for testing.
///
/// This function attempts to create a temporary MySQL server using `mysqld`.
/// The server will be automatically stopped and cleaned up when the returned
/// `MySqlSetupData` is dropped.
///
/// # Errors
///
/// Returns an error if MySQL is not installed or if server initialization fails.
pub fn mysql_tmp_server_create() -> Result<MySqlSetupData, MySqlTemporaryServerError> {
    // Clean up any orphaned processes only once per test session
    CLEANUP_ONCE.call_once(|| {
        cleanup_orphaned_mysql_processes();
    });

    // Check if mysqld is available
    if which::which("mysqld").is_err() {
        return Err(MySqlTemporaryServerError::MySqlNotFound);
    }

    // Create temporary directory for MySQL data
    let data_dir = TempDir::new()?;
    let data_path = data_dir.path();

    log::info!("Initializing MySQL in temporary directory: {:?}", data_path);

    // Initialize MySQL data directory
    let init_output = Command::new("mysqld")
        .args([
            "--initialize-insecure",
            "--datadir",
            data_path.to_str().unwrap(),
        ])
        .output()?;

    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        return Err(MySqlTemporaryServerError::InitFailed(stderr.to_string()));
    }

    // Remove any auto-generated undo files that might conflict
    // MySQL 8.0 might create these during initialization
    let _ = std::fs::remove_file(data_path.join("undo_001"));
    let _ = std::fs::remove_file(data_path.join("undo_002"));
    let _ = std::fs::remove_file(data_path.join("undo001"));
    let _ = std::fs::remove_file(data_path.join("undo002"));

    // Find an available port
    let port = find_available_port();
    let socket_path = data_path.join("mysql.sock");

    log::info!(
        "Starting MySQL server on port {} with socket {:?}",
        port,
        socket_path
    );

    // Start MySQL server
    let server_cmd = Command::new("mysqld")
        .current_dir(data_path) // Set working directory to data directory
        .args([
            "--datadir",
            data_path.to_str().unwrap(),
            "--port",
            &port.to_string(),
            "--socket",
            socket_path.to_str().unwrap(),
            "--pid-file",
            data_path.join("mysql.pid").to_str().unwrap(),
            "--skip-networking=0",
            "--bind-address=127.0.0.1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut server_cmd = match server_cmd {
        Ok(cmd) => cmd,
        Err(e) => {
            return Err(MySqlTemporaryServerError::ServerStartFailed(format!(
                "Failed to spawn mysqld: {}",
                e
            )));
        }
    };

    // Wait for server to be ready
    let mut attempts = 0;
    let max_attempts = 30;
    loop {
        if attempts >= max_attempts {
            // Check if process has exited
            if let Ok(Some(status)) = server_cmd.try_wait() {
                let mut stderr_output = String::new();
                if let Some(ref mut stderr) = server_cmd.stderr {
                    use std::io::Read;
                    let _ = stderr.read_to_string(&mut stderr_output);
                }
                let _ = server_cmd.kill();
                return Err(MySqlTemporaryServerError::ServerStartFailed(format!(
                    "Server exited with status: {}. Stderr: {}",
                    status, stderr_output
                )));
            }
            let _ = server_cmd.kill();
            return Err(MySqlTemporaryServerError::ServerStartFailed(
                "Server did not become ready in time".to_string(),
            ));
        }

        // Check if process has already exited
        if let Ok(Some(status)) = server_cmd.try_wait() {
            let mut stderr_output = String::new();
            if let Some(ref mut stderr) = server_cmd.stderr {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut stderr_output);
            }
            return Err(MySqlTemporaryServerError::ServerStartFailed(format!(
                "Server exited prematurely with status: {}. Stderr: {}",
                status, stderr_output
            )));
        }

        // Try to connect using mysqladmin
        let ping = Command::new("mysqladmin")
            .args([
                "ping",
                "-h",
                "127.0.0.1",
                "-P",
                &port.to_string(),
                "-u",
                "root",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if let Ok(status) = ping {
            if status.success() {
                log::info!("MySQL server is ready");
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
        attempts += 1;
    }

    // Create test database
    let create_db = Command::new("mysql")
        .args([
            "-h",
            "127.0.0.1",
            "-P",
            &port.to_string(),
            "-u",
            "root",
            "-e",
            "CREATE DATABASE IF NOT EXISTS test;",
        ])
        .output()?;

    if !create_db.status.success() {
        let stderr = String::from_utf8_lossy(&create_db.stderr);
        let _ = server_cmd.kill();
        return Err(MySqlTemporaryServerError::ServerStartFailed(format!(
            "Failed to create test database: {}",
            stderr
        )));
    }

    Ok(MySqlSetupData {
        _data_dir: data_dir,
        port,
        socket: Some(socket_path),
        server_process: Some(server_cmd),
    })
}

/// Find an available port for the MySQL server.
fn find_available_port() -> u16 {
    // Try to bind to port 0 to let the OS choose an available port
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port 0");
    let port = listener
        .local_addr()
        .expect("Failed to get local address")
        .port();
    drop(listener);
    port
}

/// Kill any orphaned MySQL test processes.
/// This is a utility function to clean up any MySQL processes that weren't properly terminated.
/// It tries to be smart about not killing processes that are actively being used by running tests.
pub fn cleanup_orphaned_mysql_processes() {
    use std::process::Command;

    log::info!("Cleaning up any orphaned MySQL test processes");

    // Find MySQL processes running in temp directories
    let output = Command::new("ps").args(["aux"]).output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);

        // Collect process info including data directory
        let mysql_processes: Vec<(u32, String)> = output_str
            .lines()
            .filter(|line| {
                line.contains("mysqld")
                    && (line.contains("/tmp")
                        || line.contains("/var/folders/")
                        || line.contains(".tmp"))
            })
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        // Extract the full command line to get the data directory
                        let command_line = line
                            .split_whitespace()
                            .skip(10)
                            .collect::<Vec<_>>()
                            .join(" ");
                        return Some((pid, command_line));
                    }
                }
                None
            })
            .collect();

        for (pid, command_line) in mysql_processes {
            // Extract data directory from command line
            let data_dir = if let Some(datadir_pos) = command_line.find("--datadir") {
                let after_datadir = &command_line[datadir_pos + 10..];
                if let Some(space_pos) = after_datadir.find(' ') {
                    Some(&after_datadir[..space_pos])
                } else {
                    Some(after_datadir)
                }
            } else {
                None
            };

            // Check if the data directory still exists - if not, the process is likely orphaned
            let is_orphaned = if let Some(dir) = data_dir {
                !std::path::Path::new(dir).exists()
            } else {
                // If we can't determine the data directory, be conservative and don't kill
                false
            };

            if is_orphaned {
                log::warn!(
                    "Killing orphaned MySQL process with PID: {} (data dir: {:?})",
                    pid,
                    data_dir
                );
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()]) // Try graceful termination first
                    .output();

                // Give it a moment to terminate gracefully
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Check if it's still running and force kill if necessary
                let check_output = Command::new("ps").args(["-p", &pid.to_string()]).output();

                if let Ok(check) = check_output {
                    if check.status.success() {
                        log::warn!("Process {} still running, force killing", pid);
                        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                    }
                }
            } else {
                log::debug!(
                    "MySQL process {} appears to be active (data dir exists: {:?})",
                    pid,
                    data_dir
                );
            }
        }
    }
}

impl Drop for MySqlSetupData {
    fn drop(&mut self) {
        log::info!("Shutting down temporary MySQL server on port {}", self.port);

        // First try to shutdown MySQL gracefully
        let graceful_shutdown = Command::new("mysqladmin")
            .args([
                "shutdown",
                "-h",
                "127.0.0.1",
                "-P",
                &self.port.to_string(),
                "-u",
                "root",
            ])
            .output();

        if let Ok(output) = graceful_shutdown {
            if output.status.success() {
                log::info!("MySQL server gracefully shut down");
                // Give it a moment to shutdown
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Check if process has exited
                if let Some(ref mut process) = self.server_process {
                    if let Ok(Some(_)) = process.try_wait() {
                        log::info!("MySQL process has exited");
                        return;
                    }
                }
            }
        }

        // If graceful shutdown failed, forcefully kill the process
        if let Some(ref mut process) = self.server_process {
            log::warn!("Graceful shutdown failed, forcefully killing MySQL process");
            let _ = process.kill();
            // Wait for process to actually terminate
            let _ = process.wait();
            log::info!("MySQL process forcefully terminated");
        }
    }
}
