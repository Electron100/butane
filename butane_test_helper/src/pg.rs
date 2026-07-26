//! PostgreSQL test server management.
//!
//! This module provides functionality to create temporary PostgreSQL servers for testing.
//! It supports two backends:
//! - ephemeralpg's `pg_tmp` command (preferred if available)
//! - Manual server creation using `initdb` and `postgres` (fallback)
//!
//! # Running as root
//!
//! PostgreSQL refuses to run `initdb` and the server as root. When the effective UID is 0
//! (e.g. in containers), this module drops privileges to an unprivileged system user
//! (`postgres`, then `nobody`) for those subprocesses, so `cargo test` can still be invoked
//! as root. Connection URIs are adjusted so the (still-root) test process can authenticate.
//!
//! Because the subprocesses run as that unprivileged user, the paths they touch must be
//! reachable by it: the data directory (created under the current working directory as
//! `tmp_pg/<id>`) and the socket directory (under `TMPDIR`) must have every parent
//! traversable by the drop-to user. This bites when the checkout lives under a private
//! home (`/Users/<u>` at `0750` on macOS, or `/root` at `0700` on Linux); run from a
//! world-traversable location such as `/tmp` in that case.
//!
//! # Locale and encoding
//!
//! `initdb` is invoked with `-E UTF8`. Subprocesses are given `LC_ALL=C` when the locale
//! environment is unset or names an uninstalled locale (see [`ensure_pg_locale_env`]).
//!
//! # Locating binaries
//!
//! PostgreSQL binaries are frequently installed outside `PATH` (distro version dirs,
//! Homebrew kegs). [`find_pg_binary`] resolves them without mutating the process
//! environment, and children spawned via this module get a `PATH` that includes the
//! discovered directories so wrappers like `pg_tmp` can locate `initdb`.

use std::io::{BufRead, BufReader, Read};
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Global mutex to serialize pg_tmp calls to avoid race conditions.
///
/// ephemeralpg's pg_tmp has internal state that can conflict when called concurrently.
static PG_TMP_LOCK: Mutex<()> = Mutex::new(());

/// Unprivileged OS user to run PostgreSQL processes as (when the test process is root).
#[cfg(unix)]
#[derive(Clone, Debug)]
pub(crate) struct PgOsUser {
    /// Numeric user id.
    pub uid: u32,
    /// Numeric group id.
    pub gid: u32,
    /// Account name (used for logging, `USER`/`LOGNAME`, and the cluster role).
    pub name: String,
}

/// Return the current effective user ID.
#[cfg(unix)]
pub(crate) fn current_euid() -> u32 {
    // Safety: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// Look up a local account by name via `getpwnam`.
#[cfg(unix)]
fn lookup_user(name: &str) -> Option<PgOsUser> {
    use std::ffi::CString;

    let cname = CString::new(name).ok()?;
    // Safety: getpwnam is called with a valid CString; the result is only read if non-null.
    unsafe {
        let pw = libc::getpwnam(cname.as_ptr());
        if pw.is_null() {
            return None;
        }
        Some(PgOsUser {
            uid: (*pw).pw_uid,
            gid: (*pw).pw_gid,
            name: name.to_string(),
        })
    }
}

/// If running as root, choose an unprivileged user to own PostgreSQL processes.
///
/// Prefers the `postgres` system account, then `nobody`. Returns `None` when the current
/// process is already unprivileged, or when no suitable account exists.
#[cfg(unix)]
pub(crate) fn pg_run_as_user() -> Option<PgOsUser> {
    if current_euid() != 0 {
        return None;
    }
    lookup_user("postgres").or_else(|| lookup_user("nobody"))
}

/// Resolve the run-as user, erroring if the process is root but no account is available.
#[cfg(unix)]
pub(crate) fn require_run_as_if_root() -> Result<Option<PgOsUser>, PgTemporaryServerError> {
    if current_euid() != 0 {
        return Ok(None);
    }
    match pg_run_as_user() {
        Some(user) => Ok(Some(user)),
        None => Err(PgTemporaryServerError::EphemeralPg(
            concat!(
                "Running as root, but neither the 'postgres' nor 'nobody' system user exists; ",
                "cannot drop privileges for initdb/postgres (PostgreSQL refuses to run as root)"
            )
            .to_string(),
        )),
    }
}

/// Change ownership of a path so a dropped-privilege PostgreSQL process can use it.
#[cfg(unix)]
pub(crate) fn chown_path(path: &std::path::Path, user: &PgOsUser) -> std::io::Result<()> {
    use std::os::unix::fs::chown;
    chown(path, Some(user.uid), Some(user.gid))
}

/// Whether a user with `uid`/`gid` has the execute (search, for a directory) bit on `md`.
///
/// Group membership is approximated by the primary `gid`; supplementary groups are not consulted,
/// so a path reachable only through one is reported as unreachable. Callers phrase findings as the
/// likely cause rather than a certainty for that reason.
#[cfg(unix)]
fn has_execute_bit(md: &std::fs::Metadata, uid: u32, gid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;

    // uid 0 bypasses the discretionary check entirely.
    if uid == 0 {
        return true;
    }
    let mode = md.mode();
    if md.uid() == uid {
        mode & 0o100 != 0
    } else if md.gid() == gid {
        mode & 0o010 != 0
    } else {
        mode & 0o001 != 0
    }
}

/// Name the outermost directory above `path` that a user with `uid`/`gid` cannot search.
///
/// Every parent directory of an executable must be searchable for `execve` to reach it, so a
/// single missing `x` bit anywhere on the chain makes the binary unusable by that user even when
/// the binary itself is world-executable. Returns `None` when the whole chain is searchable.
///
/// The outermost blocker is returned because that is the one to fix: making a deeper directory
/// searchable changes nothing while an ancestor still blocks the walk.
#[cfg(unix)]
#[must_use]
pub fn first_unsearchable_ancestor(path: &Path, uid: u32, gid: u32) -> Option<PathBuf> {
    // `ancestors` yields the path itself first, then each parent; skip(1) starts at the directory.
    // It is ordered innermost-first, so the last blocker found is the outermost one.
    let mut outermost = None;
    for dir in path.ancestors().skip(1) {
        let Ok(md) = std::fs::metadata(dir) else {
            continue;
        };
        if !has_execute_bit(&md, uid, gid) {
            outermost = Some(dir.to_path_buf());
        }
    }
    outermost
}

/// Explain a spawn failure that happened after dropping privileges from root.
///
/// Returns `Some(message)` only for `PermissionDenied` while privileges were dropped. `EACCES` from
/// `execve` in that situation almost never means the program is absent -- resolution already
/// succeeded, since the path came from [`find_pg_binary`] -- it means the unprivileged account
/// cannot reach or execute it. The usual cause is a binary installed under a private home
/// (`/root` at `0700`, a per-user tool prefix), which root can run but the drop-to user cannot.
#[cfg(unix)]
pub(crate) fn privilege_drop_spawn_hint(
    err: &std::io::Error,
    program: &Path,
    run_as: Option<&PgOsUser>,
) -> Option<String> {
    let user = run_as?;
    if err.kind() != std::io::ErrorKind::PermissionDenied {
        return None;
    }
    Some(describe_unreachable_binary(
        program, &user.name, user.uid, user.gid,
    ))
}

/// Describe why `program` is not executable by the account with `user_name`/`uid`/`gid`.
///
/// Split out from the crate-internal spawn-failure hint so the wording can be exercised directly
/// without provoking a real `EACCES` from a privilege-dropped spawn.
#[cfg(unix)]
#[must_use]
pub fn describe_unreachable_binary(program: &Path, user_name: &str, uid: u32, gid: u32) -> String {
    let user = PgOsUser {
        uid,
        gid,
        name: user_name.to_string(),
    };
    let mut msg = format!(
        concat!(
            "Permission denied executing `{}` as user `{}` (uid {}).\n  ",
            "PostgreSQL refuses to run as root, so this process dropped privileges to that ",
            "account, but it cannot execute that path.",
        ),
        program.display(),
        user.name,
        user.uid
    );

    if let Some(blocker) = first_unsearchable_ancestor(program, user.uid, user.gid) {
        let mode = std::fs::metadata(&blocker)
            .map(|md| {
                use std::os::unix::fs::MetadataExt;
                format!("mode {:04o}, owned by uid {}", md.mode() & 0o7777, md.uid())
            })
            .unwrap_or_else(|_| "mode unknown".to_string());
        msg.push_str(&format!(
            concat!(
                "\n  Likely cause: `{}` is not searchable by `{}` ({}), so every path below ",
                "it is unreachable.",
            ),
            blocker.display(),
            user.name,
            mode
        ));
    } else if std::fs::metadata(program).is_ok_and(|md| !has_execute_bit(&md, user.uid, user.gid)) {
        msg.push_str(&format!(
            "\n  Likely cause: the binary itself is not executable by `{}`.",
            user.name
        ));
    }

    msg.push_str(concat!(
        "\n  Fix: install the binary somewhere world-traversable (e.g. `/usr/local/bin`), ",
        "or grant traversal on the blocking directory (`chmod o+x <dir>`).",
    ));
    msg
}

/// Apply UID/GID and a usable environment so a child runs as `user`.
///
/// Root's `HOME` may be unreadable after `setuid`, so a per-user scratch home under `/tmp`
/// is created and used instead (`nobody` typically has a non-existent home).
#[cfg(unix)]
pub(crate) fn apply_run_as(cmd: &mut Command, user: &PgOsUser) {
    use std::os::unix::process::CommandExt;

    // uid/gid are applied after fork, before exec.
    cmd.uid(user.uid);
    cmd.gid(user.gid);

    // skipcq: RS-S1003
    let home = format!("/tmp/butane-pg-home-{}", user.name);
    let _ = std::fs::create_dir_all(&home);
    let _ = chown_path(std::path::Path::new(&home), user);
    cmd.env("HOME", &home);
    cmd.env("USER", &user.name);
    cmd.env("LOGNAME", &user.name);
}

/// Whether the process locale environment actually resolves to an installed locale.
///
/// `newlocale` with an empty name resolves exactly as `initdb`'s `setlocale(LC_ALL, "")` does, and
/// unlike `setlocale` it does not mutate global state. A minimal install commonly has `LANG` set to
/// a locale whose data was never generated, which is what makes `initdb` abort.
#[cfg(unix)]
fn locale_env_is_usable() -> bool {
    let Ok(empty) = std::ffi::CString::new("") else {
        return true;
    };
    // Safety: an empty locale name reads the environment; a null base requests a fresh locale.
    unsafe {
        let loc = libc::newlocale(libc::LC_ALL_MASK, empty.as_ptr(), std::ptr::null_mut());
        if loc.is_null() {
            return false;
        }
        libc::freelocale(loc);
        true
    }
}

#[cfg(not(unix))]
fn locale_env_is_usable() -> bool {
    true
}

/// Set `LC_ALL=C` when the locale environment is unset, or set to a locale that is not installed.
///
/// Empty values (e.g. `LANG=`) count as unset. `LC_ALL` wins over an inherited bad `LANG`.
pub fn ensure_pg_locale_env(cmd: &mut Command) {
    let has_locale = ["LC_ALL", "LANG", "LC_CTYPE"]
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|v| !v.is_empty()));
    if !has_locale || !locale_env_is_usable() {
        cmd.env("LC_ALL", "C");
    }
}

/// Ensure a PostgreSQL URI includes a username.
///
/// pg_tmp's unix-socket URI form is `postgresql:///test?host=...` with no user, relying on
/// peer auth matching the OS user to a DB role. When the server was started as another OS
/// user (privilege drop from root), the connecting root process must name the role that owns
/// the cluster explicitly.
pub fn ensure_uri_has_user(uri: &str, username: &str) -> String {
    // Authority form already has userinfo: postgresql://user@host/db
    if let Some(scheme_end) = uri.find("://") {
        let after_scheme = &uri[scheme_end + 3..];
        // The authority is everything up to the first '/' or '?'.
        let authority = after_scheme
            .split(['/', '?'])
            .next()
            .unwrap_or(after_scheme);
        if authority.contains('@') {
            return uri.to_string();
        }
    }

    if uri.split(['?', '&']).any(|p| p.starts_with("user=")) {
        return uri.to_string();
    }

    if uri.contains('?') {
        format!("{uri}&user={username}")
    } else {
        format!("{uri}?user={username}")
    }
}

/// Roots holding versioned PostgreSQL install prefixes.
///
/// Each entry pairs a root with the directory-name prefix that precedes the major version.
/// Each version subdirectory contains a `bin/`. Any installed version is discovered by
/// scanning these roots, so new PostgreSQL releases are picked up without code changes.
const PG_VERSIONED_ROOTS: &[(&str, &str)] = &[
    ("/usr/lib/postgresql", ""), // Debian/Ubuntu: /usr/lib/postgresql/<N>/bin
    ("/usr", "pgsql-"),          // RHEL/RPM:      /usr/pgsql-<N>/bin
    ("/opt/homebrew/opt", "postgresql@"), // Homebrew (Apple silicon)
    ("/usr/local/opt", "postgresql@"), // Homebrew (Intel)
];

/// Unversioned install prefixes (e.g. source builds).
const PG_FIXED_BINDIRS: &[&str] = &["/usr/local/pgsql/bin"];

/// Discover PostgreSQL `bin` directories, newest major version first.
///
/// Scans [`PG_VERSIONED_ROOTS`] for version subdirectories, sorts them by major version
/// descending, then appends [`PG_FIXED_BINDIRS`].
fn discover_pg_bindirs() -> Vec<std::path::PathBuf> {
    let mut versioned: Vec<(u32, std::path::PathBuf)> = Vec::new();
    for (root, prefix) in PG_VERSIONED_ROOTS {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(version) = name
                .to_string_lossy()
                .strip_prefix(prefix)
                .map(str::to_owned)
            else {
                continue;
            };
            // Leading digits are the major version (e.g. "16", "16.2", "pgsql-15" -> "15").
            let major: u32 = version
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let bindir = entry.path().join("bin");
            if bindir.is_dir() {
                versioned.push((major, bindir));
            }
        }
    }
    versioned.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut dirs: Vec<std::path::PathBuf> = versioned.into_iter().map(|(_, dir)| dir).collect();
    dirs.extend(PG_FIXED_BINDIRS.iter().map(std::path::PathBuf::from));
    dirs
}

/// Locate a PostgreSQL-related binary without mutating the process environment.
///
/// Resolution order:
/// 1. `PATH` via [`which::which`]
/// 2. Discovered install prefixes (`discover_pg_bindirs`), newest version first
///
/// Set `BUTANE_PG_NO_CANDIDATE_PATHS=1` to disable step 2 (used by tests that clear `PATH`
/// to simulate missing tools).
pub fn find_pg_binary(name: &str) -> Option<std::path::PathBuf> {
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    if std::env::var_os("BUTANE_PG_NO_CANDIDATE_PATHS").is_some() {
        return None;
    }
    for dir in discover_pg_bindirs() {
        let candidate = dir.join(name);
        if candidate.is_file() {
            log::debug!("found {} at {}", name, candidate.display());
            return Some(candidate);
        }
    }
    None
}

/// Build a child-process `PATH` that includes the discovered PG binary directories.
///
/// This lets shell wrappers like `pg_tmp` find `initdb`/`postgres`.
fn child_path_with_pg_bins() -> std::ffi::OsString {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for name in ["initdb", "postgres", "pg_ctl", "pg_tmp"] {
        if let Some(parent) = find_pg_binary(name).and_then(|b| b.parent().map(|p| p.to_path_buf()))
        {
            if !dirs.contains(&parent) {
                dirs.push(parent);
            }
        }
    }
    let current = std::env::var_os("PATH").unwrap_or_default();
    if dirs.is_empty() {
        return current;
    }
    let mut new_path = std::ffi::OsString::new();
    for (i, dir) in dirs.iter().enumerate() {
        if i > 0 {
            new_path.push(":");
        }
        new_path.push(dir.as_os_str());
    }
    if !current.is_empty() {
        new_path.push(":");
        new_path.push(current);
    }
    new_path
}

/// Configure a child PostgreSQL command's environment.
///
/// Sets a `PATH` that exposes the PG binaries to shell wrappers, and `LC_ALL=C` when the
/// locale environment is unset (see [`ensure_pg_locale_env`]). Privilege dropping is applied
/// separately via [`apply_run_as`].
pub(crate) fn configure_pg_command_env(cmd: &mut Command) {
    cmd.env("PATH", child_path_with_pg_bins());
    ensure_pg_locale_env(cmd);
}

/// Parse the SysV shared-memory segment id from `postmaster.pid` contents.
///
/// The id is on line 7 (`LOCK_FILE_LINE_SHMEM_KEY`), formatted `<key> <id>`, and its `<id>` is
/// the `ID` column of `ipcs -m`. Returns `Ok(Some(id))` for a live segment, `Ok(None)` when no
/// SysV segment is recorded (id 0), and `Err` with a diagnostic when the line is missing or is
/// not two integers -- most likely because PostgreSQL changed the `postmaster.pid` format and
/// this code needs updating.
#[cfg(target_os = "macos")]
pub fn shmem_id_from_postmaster_pid(content: &str) -> Result<Option<u64>, String> {
    // The 7th line of postmaster.pid (LOCK_FILE_LINE_SHMEM_KEY) is "<key> <id>".
    const SHMEM_LINE: usize = 6;

    let lines: Vec<&str> = content.lines().collect();
    let shmem_line = lines.get(SHMEM_LINE).ok_or_else(|| {
        format!(
            concat!(
                "postmaster.pid has {} lines, expected the shared-memory key on line {} ",
                "(postmaster.pid format may have changed)"
            ),
            lines.len(),
            SHMEM_LINE + 1,
        )
    })?;

    // Expect exactly two integers, "<key> <id>"; anything else means the format changed.
    let mut fields = shmem_line.split_whitespace();
    let (Some(key), Some(id), None) = (fields.next(), fields.next(), fields.next()) else {
        return Err(format!(
            concat!(
                "shared-memory line {} is {:?}, expected two integers `<key> <id>` ",
                "(postmaster.pid format may have changed)"
            ),
            SHMEM_LINE + 1,
            shmem_line,
        ));
    };
    // Both fields must be integers (the key is validated but only the id locates the segment).
    let (Ok(_), Ok(id)) = (key.parse::<u64>(), id.parse::<u64>()) else {
        return Err(format!(
            concat!(
                "shared-memory line {} {:?} is not two integers `<key> <id>` ",
                "(postmaster.pid format may have changed)"
            ),
            SHMEM_LINE + 1,
            shmem_line,
        ));
    };

    Ok((id != 0).then_some(id))
}

/// Remove the SysV shared-memory segment recorded in `data_dir`'s `postmaster.pid`.
///
/// macOS keeps orphaned segments when postgres does not shut down cleanly. The segment is
/// identified via [`shmem_id_from_postmaster_pid`].
///
/// Returns `Ok(true)` when a segment was removed, `Ok(false)` when there is nothing to do (no
/// `postmaster.pid`, no segment allocated, or it is already gone), and `Err` with a diagnostic
/// when the file cannot be read or interpreted.
#[cfg(target_os = "macos")]
pub fn cleanup_macos_postgres_shared_memory(data_dir: &std::path::Path) -> Result<bool, String> {
    let pid_file = data_dir.join("postmaster.pid");
    let content = match std::fs::read_to_string(&pid_file) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::debug!("{} not found; nothing to clean", pid_file.display());
            return Ok(false);
        }
        Err(e) => return Err(format!("failed to read {}: {e}", pid_file.display())),
    };

    let Some(id) = shmem_id_from_postmaster_pid(&content)
        .map_err(|e| format!("{}: {e}", pid_file.display()))?
    else {
        log::debug!("no SysV shared-memory segment recorded; nothing to clean");
        return Ok(false);
    };

    let id = id.to_string();
    if !shmem_segment_exists(&id)? {
        log::debug!("shared-memory segment {id} already gone");
        return Ok(false);
    }

    log::info!("removing orphaned shared-memory segment {id}");
    let output = Command::new("ipcrm")
        .arg("-m")
        .arg(&id)
        .output()
        .map_err(|e| format!("failed to run `ipcrm -m {id}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`ipcrm -m {id}` failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    log::info!("removed shared-memory segment {id}");
    Ok(true)
}

/// Whether a SysV shared-memory segment with `id` appears in the `ID` column of `ipcs -m`.
#[cfg(target_os = "macos")]
fn shmem_segment_exists(id: &str) -> Result<bool, String> {
    let output = Command::new("ipcs")
        .arg("-m")
        .output()
        .map_err(|e| format!("failed to run `ipcs -m`: {e}"))?;
    let listing = String::from_utf8(output.stdout)
        .map_err(|e| format!("`ipcs -m` output was not UTF-8: {e}"))?;
    Ok(listing.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("m") && fields.next() == Some(id)
    }))
}

/// Error related to the temporary PostgreSQL server.
#[derive(Debug, thiserror::Error)]
pub enum PgTemporaryServerError {
    /// IO errors.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Error parsing pg_tmp output.
    #[error("Failed to parse pg_tmp output: {0}")]
    Parse(String),
    /// pg_tmp command failed.
    #[error("pg_tmp command failed: {0}")]
    EphemeralPg(String),
    /// A PostgreSQL binary could not be executed by the account root dropped privileges to.
    ///
    /// Distinct from [`Self::Io`] so callers can tell "the environment cannot support the
    /// privilege drop" apart from an ordinary IO failure; the message names the blocking path.
    #[error("{0}")]
    PrivilegeDrop(String),
}

/// Check if pg_tmp (ephemeralpg) is available.
pub fn is_pg_tmp_available() -> bool {
    find_pg_binary("pg_tmp").is_some()
}

/// Check if initdb is available.
pub fn is_initdb_available() -> bool {
    find_pg_binary("initdb").is_some()
}

/// Check if the `postgres` server binary is available.
pub fn is_postgres_available() -> bool {
    find_pg_binary("postgres").is_some()
}

/// Create a temporary PostgreSQL server using ephemeralpg's pg_tmp command.
///
/// This function is thread-safe - it uses a global mutex to serialize calls to pg_tmp
/// to avoid race conditions in ephemeralpg's internal state management.
///
/// When the current process is root, `pg_tmp` (and thus `initdb`) is executed as an
/// unprivileged user (see `pg_run_as_user`) because PostgreSQL refuses to run as root.
/// The returned URI is adjusted to include that username so the root test process can
/// connect. `LC_ALL=C` is supplied to the subprocess when the locale environment is unset
/// (see [`ensure_pg_locale_env`]).
pub fn pg_tmp_server_create_ephemeralpg(
    options: crate::PgServerOptions,
) -> Result<crate::PgServerState, PgTemporaryServerError> {
    // Acquire lock to serialize pg_tmp calls across threads
    let _lock = PG_TMP_LOCK.lock().unwrap();

    #[cfg(unix)]
    let run_as = require_run_as_if_root()?;
    #[cfg(unix)]
    if let Some(ref user) = run_as {
        log::info!(
            "running as root: executing pg_tmp as user {} (uid {})",
            user.name,
            user.uid
        );
    }

    let pg_tmp = find_pg_binary("pg_tmp").ok_or_else(|| {
        PgTemporaryServerError::EphemeralPg(
            "Failed to spawn pg_tmp (is ephemeralpg installed?): program not found".to_string(),
        )
    })?;
    let mut command = Command::new(&pg_tmp);

    // Add wait time option if specified
    if let Some(wait_seconds) = options.ephemeralpg_wait_seconds {
        command.arg("-w").arg(wait_seconds.to_string());
    }

    // Add custom postgres options if specified
    if let Some(port) = options.port {
        command.arg("-o").arg(format!("-p {}", port));
    }

    // Ensure initdb is visible to pg_tmp and a locale is set, then drop root if needed.
    configure_pg_command_env(&mut command);
    #[cfg(unix)]
    if let Some(ref user) = run_as {
        apply_run_as(&mut command, user);
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    log::info!("spawning pg_tmp...");
    let mut proc = command.spawn().map_err(|e| {
        // A privilege-dropped EACCES is a reachability problem, not a missing install; saying
        // "is ephemeralpg installed?" there sends the reader looking for the wrong thing.
        #[cfg(unix)]
        if let Some(hint) = privilege_drop_spawn_hint(&e, &pg_tmp, run_as.as_ref()) {
            return PgTemporaryServerError::PrivilegeDrop(hint);
        }
        PgTemporaryServerError::EphemeralPg(format!(
            "Failed to spawn pg_tmp (is ephemeralpg installed?): {}",
            e
        ))
    })?;

    // Read the connection URI from stdout
    let mut stdout = BufReader::new(proc.stdout.take().unwrap());
    let mut uri = String::new();
    stdout.read_line(&mut uri).map_err(|e| {
        PgTemporaryServerError::EphemeralPg(format!("Failed to read pg_tmp output: {}", e))
    })?;

    let uri = uri.trim();
    if uri.is_empty() {
        // Check if process died
        if let Some(status) = proc.try_wait().map_err(PgTemporaryServerError::Io)? {
            let mut stderr = BufReader::new(proc.stderr.take().unwrap());
            let mut error_msg = String::new();
            stderr.read_to_string(&mut error_msg).ok();
            return Err(PgTemporaryServerError::EphemeralPg(format!(
                "pg_tmp exited with status {}: {}",
                status, error_msg
            )));
        }
        return Err(PgTemporaryServerError::Parse(
            "pg_tmp returned empty URI".to_string(),
        ));
    }

    // Socket URIs from pg_tmp omit the username; when the server was started as another OS
    // user (privilege drop from root), the only role is that user, so inject it explicitly
    // for the still-root connecting process.
    #[cfg(unix)]
    let uri = if let Some(ref user) = run_as {
        ensure_uri_has_user(uri, &user.name)
    } else {
        uri.to_string()
    };
    #[cfg(not(unix))]
    let uri = uri.to_string();

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
        ephemeralpg_uri: Some(uri),
    })
}
