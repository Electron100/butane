//! Integration tests for server creation with the locale environment (`LANG` / `LC_*`) cleared.
#![cfg(test)]
#![cfg(feature = "pg")]
#![cfg(not(target_os = "windows"))]

use butane_test_helper::{
    is_initdb_available, is_pg_tmp_available, is_postgres_available,
    pg_tmp_server_create_ephemeralpg, pg_tmp_server_create_using_initdb, PgServerOptions,
};

/// Locale environment variables cleared for these tests.
const LOCALE_VARS: &[&str] = &[
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_COLLATE",
    "LC_MESSAGES",
    "LC_MONETARY",
    "LC_NUMERIC",
    "LC_TIME",
];

fn cleared_locale_env() -> Vec<(&'static str, Option<&'static str>)> {
    LOCALE_VARS.iter().map(|&k| (k, None)).collect()
}

/// `initdb`-based server creation must succeed with the locale environment unset.
#[test]
fn initdb_without_locale_env() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found");
        return;
    }

    temp_env::with_vars(cleared_locale_env(), || {
        let result = pg_tmp_server_create_using_initdb(PgServerOptions::default());
        assert!(
            result.is_ok(),
            "initdb server creation should succeed with locale unset: {:?}",
            result.err()
        );
    });
}

/// ephemeralpg (`pg_tmp`) server creation must succeed with the locale environment unset.
#[test]
fn ephemeralpg_without_locale_env() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found");
        return;
    }

    temp_env::with_vars(cleared_locale_env(), || {
        let options = PgServerOptions {
            ephemeralpg_wait_seconds: Some(120),
            ..Default::default()
        };
        let result = pg_tmp_server_create_ephemeralpg(options);
        assert!(
            result.is_ok(),
            "ephemeralpg server creation should succeed with locale unset: {:?}",
            result.err()
        );
        let server = result.unwrap();
        assert!(
            server.ephemeralpg_uri.is_some(),
            "ephemeralpg server should expose a URI"
        );
    });
}
