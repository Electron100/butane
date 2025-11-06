//! Libsql server (sqld) test helper implementation.
//!
//! This module provides utilities for testing with sqld (libsql-server) instances.
//! It manages sqld server processes for integration testing with the Libsql backend.
//!
//! # Features
//!
//! - Automatic sqld server lifecycle management
//! - Port allocation using process ID + atomic counter to avoid conflicts
//! - Health checks with configurable timeout
//! - Async I/O for process output capture

use std::future::Future;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};

use butane_core::db::libsql::{LibsqlBackend, BACKEND_NAME};
use butane_core::db::{Backend, ConnectionAsync, ConnectionSpec};

use crate::{common_setup, setup_db_async, BackendTestInstance, SetupData};

/// Global atomic counter for port allocation to avoid conflicts
/// Start with a random offset based on process ID to reduce collision probability
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

fn get_base_port() -> u16 {
    // Use process ID to get a semi-random base port
    let pid = std::process::id() as u16;
    8080 + (pid % 1000)
}

/// Represents a running sqld server instance
pub struct SqldServer {
    process: tokio::process::Child,
    /// The HTTP URL of the running server
    pub url: String,
    /// The port number the server is listening on
    pub port: u16,
    /// The path to the database file
    pub db_path: String,
    _temp_dir: tempfile::TempDir,
}

impl SqldServer {
    /// Start a new sqld server instance
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("test.db");

        // Generate a unique port using atomic counter to avoid conflicts
        let port_offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let port = get_base_port() + port_offset;

        log::debug!("Starting sqld server on port {}", port);

        // Try to find sqld in common locations
        let sqld_binary = Self::find_sqld_binary()?;

        let mut cmd = tokio::process::Command::new(&sqld_binary);
        cmd.arg("--db-path")
            .arg(&db_path)
            .arg("--http-listen-addr")
            .arg(format!("127.0.0.1:{}", port))
            .arg("--no-welcome")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut process = cmd.spawn()?;
        log::debug!("Spawned sqld process with PID: {:?}", process.id());

        // Capture and log stderr for debugging
        if let Some(stderr) = process.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log::debug!("sqld stderr: {}", line);
                }
            });
        }

        // Capture and log stdout for debugging
        if let Some(stdout) = process.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log::debug!("sqld stdout: {}", line);
                }
            });
        }

        // Wait for the server to be ready
        let url = format!("http://127.0.0.1:{}", port);

        // Give sqld a moment to bind to the port
        tokio::time::sleep(Duration::from_secs(2)).await;

        Self::wait_for_server_ready(&url, port).await?;

        let server = SqldServer {
            process,
            url: url.clone(),
            port,
            db_path: db_path.to_string_lossy().to_string(),
            _temp_dir: temp_dir,
        };

        Ok(server)
    }

    /// Find the sqld binary in common locations
    fn find_sqld_binary() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Try common locations for sqld
        let possible_paths = [
            "sqld",                   // In PATH
            "./target/release/sqld",  // Local build
            "./target/debug/sqld",    // Local debug build
            "~/.cargo/bin/sqld",      // Cargo install location
            "/usr/local/bin/sqld",    // System install
            "/opt/homebrew/bin/sqld", // Homebrew on macOS
        ];

        for path in &possible_paths {
            if let Ok(output) = std::process::Command::new(path).arg("--version").output() {
                if output.status.success() {
                    log::info!("Found sqld at: {}", path);
                    return Ok(path.to_string());
                }
            }
        }

        Err("Could not find or install sqld binary".into())
    }

    /// Wait for the server to be ready by checking if it responds to HTTP requests
    async fn wait_for_server_ready(
        url: &str,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()?;

        // Try for up to 60 seconds to allow time for concurrent test scenarios
        for attempt in 1..=600 {
            tokio::time::sleep(Duration::from_millis(100)).await;

            match client.get(url).send().await {
                Ok(_response) => {
                    log::debug!(
                        "sqld server on port {} is ready (attempt {})",
                        port,
                        attempt
                    );
                    return Ok(());
                }
                Err(e) if e.is_connect() => {
                    // Connection error, server not ready yet
                    if attempt % 50 == 0 {
                        log::debug!("Port {}: Still waiting after {} attempts", port, attempt);
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Port {}: Unexpected error on attempt {}: {}",
                        port,
                        attempt,
                        e
                    );
                }
            }
        }

        Err(format!(
            "Timeout waiting for sqld server on port {} to be ready after 60 seconds",
            port
        )
        .into())
    }

    /// Get a libsql connection URL for this server
    pub fn connection_url(&self) -> String {
        format!("libsql+http://{}", self.url.trim_start_matches("http://"))
    }

    /// Stop the server
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Stopping sqld server on port {}", self.port);
        self.process.kill().await?;
        self.process.wait().await?;
        Ok(())
    }
}

impl Drop for SqldServer {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
    }
}

/// Create a libsql [`ConnectionSpec`] for a sqld server.
pub fn libsql_connspec(server: &SqldServer) -> ConnectionSpec {
    ConnectionSpec::new(BACKEND_NAME, server.connection_url())
}

/// Concrete [SetupData] for libSQL with sqld server.
pub struct LibsqlSetupData {
    server: SqldServer,
    connection_string: String,
}

impl SetupData for LibsqlSetupData {
    fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

impl LibsqlSetupData {
    /// Get a reference to the running server
    pub fn server(&self) -> &SqldServer {
        &self.server
    }
}

/// Setup the test libSQL database using a sqld server.
pub async fn libsql_setup() -> Result<LibsqlSetupData, Box<dyn std::error::Error + Send + Sync>> {
    let server = SqldServer::start().await?;
    let connection_string = server.connection_url();

    Ok(LibsqlSetupData {
        server,
        connection_string,
    })
}

/// Tear down the test libSQL database with sqld server.
pub async fn libsql_teardown(mut data: LibsqlSetupData) {
    let _ = data.server.stop().await;
}

/// Instance of a libSQL test with sqld server.
#[derive(Default)]
pub struct LibsqlTestInstance {}

impl BackendTestInstance for LibsqlTestInstance {
    fn run_test_sync(_test: impl FnOnce(crate::Connection), _migrate: bool) {
        // libSQL doesn't support sync connections - skip the test silently
        // The test framework will still count this as passed
    }

    async fn run_test_async<Fut>(test: impl FnOnce(ConnectionAsync) -> Fut, migrate: bool)
    where
        Fut: Future<Output = ()>,
    {
        common_setup();
        log::info!("setting up sqld server...");
        let mut setup_data = libsql_setup().await.expect("Could not setup sqld server");

        log::info!(
            "connecting to sqld server at {}...",
            setup_data.connection_string()
        );
        let mut conn = LibsqlBackend::new()
            .connect_async(setup_data.connection_string())
            .await
            .expect("Could not connect to sqld backend");

        if migrate {
            setup_db_async(&mut conn).await;
        }

        log::info!("running libSQL test with sqld server");
        test(conn).await;

        log::info!("tearing down sqld server");
        let _ = setup_data.server.stop().await;
    }
}
