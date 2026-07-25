//! Unit tests for the PostgreSQL helper functions.
//!
//! Covers URI user injection and locale-environment handling.
#![cfg(test)]
#![cfg(feature = "pg")]

use std::ffi::OsStr;
use std::process::Command;

use butane_test_helper::pg::{ensure_pg_locale_env, ensure_uri_has_user};

#[test]
fn ensure_uri_has_user_adds_query_param() {
    let uri = "postgresql:///test?host=%2Ftmp%2Fephemeralpg.abc";
    assert_eq!(
        ensure_uri_has_user(uri, "postgres"),
        "postgresql:///test?host=%2Ftmp%2Fephemeralpg.abc&user=postgres"
    );
}

#[test]
fn ensure_uri_has_user_adds_question_mark() {
    let uri = "postgresql://localhost/test";
    assert_eq!(
        ensure_uri_has_user(uri, "postgres"),
        "postgresql://localhost/test?user=postgres"
    );
}

#[test]
fn ensure_uri_has_user_preserves_existing_userinfo() {
    let uri = "postgresql://postgres@127.0.0.1:5432/test";
    assert_eq!(ensure_uri_has_user(uri, "other"), uri);
}

#[test]
fn ensure_uri_has_user_preserves_existing_user_param() {
    let uri = "postgresql:///test?host=%2Ftmp%2Fx&user=postgres";
    assert_eq!(ensure_uri_has_user(uri, "other"), uri);
}

/// `LC_ALL=C` is supplied when no locale is set. Empty values count as unset.
#[test]
fn locale_env_added_when_unset() {
    temp_env::with_vars(
        [
            ("LC_ALL", None::<&str>),
            ("LANG", Some("")),
            ("LC_CTYPE", None),
        ],
        || {
            let mut cmd = Command::new("true");
            ensure_pg_locale_env(&mut cmd);
            let lc_all = cmd.get_envs().find(|(k, _)| *k == OsStr::new("LC_ALL"));
            assert_eq!(lc_all, Some((OsStr::new("LC_ALL"), Some(OsStr::new("C")))));
        },
    );
}

/// An existing locale is left untouched.
#[test]
fn locale_env_not_overridden_when_set() {
    temp_env::with_var("LANG", Some("en_US.UTF-8"), || {
        let mut cmd = Command::new("true");
        ensure_pg_locale_env(&mut cmd);
        assert!(
            cmd.get_envs().all(|(k, _)| k != OsStr::new("LC_ALL")),
            "LC_ALL should not be forced when a locale is already set"
        );
    });
}

/// A uid/gid pair belonging to no directory these tests create, so only the `other` bits apply.
/// Passed explicitly rather than read from the process, so results do not depend on who runs the
/// suite -- these cases behave identically as an ordinary user and as root.
#[cfg(unix)]
const FOREIGN: (u32, u32) = (65534, 65534);

/// Build `<tmp>/outer/inner/prog`, with every directory searchable by everyone.
///
/// The tree is rooted in `/tmp`, not the per-user temp dir: `/tmp` and its ancestors are
/// world-searchable on both Linux and macOS, whereas macOS `$TMPDIR` (under `/var/folders`)
/// is `0700` and would itself block a FOREIGN user's walk before reaching the dirs under test.
#[cfg(unix)]
fn traversable_tree() -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    // skipcq: RS-S1003
    let tmp = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
    // TempDir is created 0700, which would itself block the walk.
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let inner = tmp.path().join("outer").join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    for dir in [tmp.path().join("outer"), inner.clone()] {
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let prog = inner.join("prog");
    std::fs::write(&prog, b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&prog, std::fs::Permissions::from_mode(0o755)).unwrap();
    (tmp, prog)
}

#[cfg(unix)]
#[test]
fn no_unsearchable_ancestor_when_chain_is_world_traversable() {
    let (_tmp, prog) = traversable_tree();
    assert_eq!(
        butane_test_helper::pg::first_unsearchable_ancestor(&prog, FOREIGN.0, FOREIGN.1),
        None
    );
}

/// The regression this exists for: a binary under a `0700` directory is unreachable after a
/// privilege drop even though the binary itself is world-executable.
#[cfg(unix)]
#[test]
fn private_directory_blocks_the_walk() {
    use std::os::unix::fs::PermissionsExt;

    let (tmp, prog) = traversable_tree();
    let outer = tmp.path().join("outer");
    std::fs::set_permissions(&outer, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(
        butane_test_helper::pg::first_unsearchable_ancestor(&prog, FOREIGN.0, FOREIGN.1),
        Some(outer)
    );
}

/// With blockers at two depths, the outermost is reported -- opening only the inner one would
/// leave the path just as unreachable.
#[cfg(unix)]
#[test]
fn outermost_blocker_is_reported() {
    use std::os::unix::fs::PermissionsExt;

    let (tmp, prog) = traversable_tree();
    let outer = tmp.path().join("outer");
    let inner = outer.join("inner");
    for dir in [&inner, &outer] {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    assert_eq!(
        butane_test_helper::pg::first_unsearchable_ancestor(&prog, FOREIGN.0, FOREIGN.1),
        Some(outer)
    );
}

/// The diagnostic must name the blocking directory and the account, and must not repeat the old
/// "is ephemeralpg installed?" guess -- resolution already succeeded, so the install is fine.
#[cfg(unix)]
#[test]
fn message_names_the_blocking_directory() {
    use std::os::unix::fs::PermissionsExt;

    let (tmp, prog) = traversable_tree();
    let outer = tmp.path().join("outer");
    std::fs::set_permissions(&outer, std::fs::Permissions::from_mode(0o700)).unwrap();

    let msg = butane_test_helper::pg::describe_unreachable_binary(
        &prog, "postgres", FOREIGN.0, FOREIGN.1,
    );
    println!("--- rendered diagnostic ---\n{msg}\n---");

    assert!(
        msg.contains(&outer.display().to_string()),
        "should name the blocking directory: {msg}"
    );
    assert!(msg.contains("postgres"), "should name the account: {msg}");
    assert!(
        !msg.contains("is ephemeralpg installed"),
        "must not blame the install: {msg}"
    );
}

/// uid 0 bypasses the discretionary check, so root sees no blockers. This is why the failure
/// only appears after the privilege drop: root can run the binary, the drop-to user cannot.
#[cfg(unix)]
#[test]
fn root_is_never_blocked() {
    use std::os::unix::fs::PermissionsExt;

    let (tmp, prog) = traversable_tree();
    std::fs::set_permissions(
        tmp.path().join("outer"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();

    assert_eq!(
        butane_test_helper::pg::first_unsearchable_ancestor(&prog, 0, 0),
        None
    );
}
