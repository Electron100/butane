//! Tests for the "PostgreSQL binary is missing" error paths.
//!
//! Separate binary (hence separate process) because these blank `PATH` process-wide,
//! which would otherwise break concurrent tests that need a real one.
#![cfg(test)]
#![cfg(feature = "pg")]

use butane_test_helper::{
    pg_tmp_server_create_ephemeralpg, PgServerOptions, PgTemporaryServerError,
};

// Matches the gate on tests/initdb.rs, where these two came from.
#[cfg(not(target_os = "windows"))]
use butane_test_helper::{is_initdb_available, pg_tmp_server_create_using_initdb};

fn no_pg_binaries() -> [(&'static str, Option<&'static str>); 2] {
    [("PATH", None), ("BUTANE_PG_NO_CANDIDATE_PATHS", Some("1"))]
}

/// Test that we get a proper error when initdb is not available
#[cfg(not(target_os = "windows"))]
#[test]
fn error_when_initdb_not_found() {
    temp_env::with_vars(no_pg_binaries(), || {
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
#[cfg(not(target_os = "windows"))]
#[test]
fn error_when_postgres_not_found() {
    if !is_initdb_available() {
        eprintln!("Skipping test: initdb not found in PATH (needed to test postgres missing)");
        return;
    }

    // Get the initdb directory (may live outside PATH on distro packages).
    let initdb_path = butane_test_helper::pg::find_pg_binary("initdb").and_then(|p| {
        p.parent()
            .map(|parent| parent.to_string_lossy().to_string())
    });

    if initdb_path.is_none() {
        eprintln!("Skipping test: could not determine initdb location");
        return;
    }

    let initdb_dir = initdb_path.unwrap();

    // Create a minimal PATH with just the initdb directory, and disable candidate-path
    // discovery so postgres is only found if it sits next to initdb.
    temp_env::with_vars(
        [
            ("PATH", Some(initdb_dir.as_str())),
            ("BUTANE_PG_NO_CANDIDATE_PATHS", Some("1")),
        ],
        || {
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
        },
    );
}

/// Test that we can detect if pg_tmp is not available
#[test]
fn error_when_pg_tmp_not_found() {
    temp_env::with_vars(no_pg_binaries(), || {
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
