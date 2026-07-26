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

/// A locale name no system has data for, reproducing a minimal install where `LANG` names a
/// locale that was never generated.
fn unavailable_locale_env() -> Vec<(&'static str, Option<&'static str>)> {
    let mut vars: Vec<_> = LOCALE_VARS
        .iter()
        .filter(|&&k| k != "LANG")
        .map(|&k| (k, None))
        .collect();
    vars.push(("LANG", Some("xx_YY.INVALID")));
    vars
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

/// `initdb`-based creation must survive `LANG` naming a locale that is not installed.
#[test]
fn initdb_with_unavailable_locale() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found");
        return;
    }

    temp_env::with_vars(unavailable_locale_env(), || {
        let result = pg_tmp_server_create_using_initdb(PgServerOptions::default());
        assert!(
            result.is_ok(),
            "initdb server creation should succeed with an uninstalled locale: {:?}",
            result.err()
        );
    });
}

/// ephemeralpg (`pg_tmp`) must survive `LANG` naming a locale that is not installed.
#[test]
fn ephemeralpg_with_unavailable_locale() {
    if !is_pg_tmp_available() {
        eprintln!("Skipping test: pg_tmp not found");
        return;
    }

    temp_env::with_vars(unavailable_locale_env(), || {
        let options = PgServerOptions {
            ephemeralpg_wait_seconds: Some(120),
            ..Default::default()
        };
        let result = pg_tmp_server_create_ephemeralpg(options);
        assert!(
            result.is_ok(),
            "ephemeralpg server creation should succeed with an uninstalled locale: {:?}",
            result.err()
        );
    });
}

/// A failed creation must not leave its `tmp_pg/` instance directory behind.
///
/// initdb exiting non-zero panics rather than returning `Err`, so the cleanup has to survive an
/// unwind; `catch_unwind` is what lets the test observe that.
#[test]
fn failed_creation_leaves_no_tmp_pg() {
    if !is_initdb_available() || !is_postgres_available() {
        eprintln!("Skipping test: initdb or postgres not found");
        return;
    }

    // Isolate the instance dir under a private root so concurrent tests writing the shared
    // `tmp_pg/` cannot perturb the before/after comparison.
    let root = tempfile::TempDir::new().unwrap();
    let before = read_instance_dirs(root.path());

    temp_env::with_vars(cleared_locale_env(), || {
        // An empty `-U` value makes initdb fail after the instance dir has been created.
        let options = PgServerOptions {
            user: Some(String::new()),
            instance_root: Some(root.path().to_path_buf()),
            ..Default::default()
        };
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = std::panic::catch_unwind(|| pg_tmp_server_create_using_initdb(options));
        std::panic::set_hook(hook);
        assert!(
            matches!(outcome, Err(_) | Ok(Err(_))),
            "expected creation to fail"
        );
    });

    let after = read_instance_dirs(root.path());
    assert_eq!(after, before, "a failed creation leaked an instance dir");
}

fn read_instance_dirs(tmp_pg: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(tmp_pg) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    dirs
}
