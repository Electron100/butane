//! MySQL test server management.
//!
//! This module provides functionality to create temporary MySQL servers for testing.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;
use thiserror::Error;

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
    _server_process: Option<std::process::Child>,
}

impl MySqlSetupData {
    /// Get the connection string for this MySQL instance.
    pub fn connection_string(&self) -> String {
        if let Some(socket) = &self.socket {
            format!(
                "mysql://root@localhost/test?socket={}",
                socket.display()
            )
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

    log::info!("Starting MySQL server on port {} with socket {:?}", port, socket_path);

    // Start MySQL server
    let server_cmd = Command::new("mysqld")
        .current_dir(data_path)  // Set working directory to data directory
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
        _server_process: Some(server_cmd),
    })
}

/// Find an available port for the MySQL server.
fn find_available_port() -> u16 {
    // Try to bind to port 0 to let the OS choose an available port
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port 0");
    let port = listener.local_addr().expect("Failed to get local address").port();
    drop(listener);
    port
}

impl Drop for MySqlSetupData {
    fn drop(&mut self) {
        log::info!("Shutting down temporary MySQL server on port {}", self.port);

        // Try to shutdown MySQL gracefully
        let _ = Command::new("mysqladmin")
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

        // Give it a moment to shutdown
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
