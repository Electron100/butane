use std::fs;

use butane_core::{
    db::{connect, connect_async, ConnectionAsync, ConnectionSpec},
    Error,
};
use butane_test_helper::*;
use butane_test_macros::butane_test;

#[butane_test(nomigrate)]
async fn connection_not_closed(conn: ConnectionAsync) {
    assert!(!conn.is_closed());
}

// The SQLite connection URI tests cover most cases described at https://www.sqlite.org/c3ref/open.html
// and https://www.sqlite.org/inmemorydb.html

#[test]
fn uri_sqlite_temporary_file() {
    let uri = "";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory() {
    let uri = ":memory:";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory_parameter_fails() {
    let uri = ":memory:?mode=ro";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    #[cfg(target_os = "windows")]
    {
        // Windows does not support creating files that start with a colon.
        let connection_error = connect(&spec).unwrap_err();
        // Rust tools can not yet detect that this variable is used in the macro below
        let _expected_error = format!("invalid uri authority: :memory:");
        eprintln!("{connection_error:?}");
        assert!(matches!(
            connection_error,
            Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        connect(&spec).unwrap();
        // connect succeeded, but became a file.
        fs::remove_file(uri).unwrap();
    }
}

#[test]
fn uri_sqlite_memory_file_scheme() {
    let uri = "file::memory:";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory_file_scheme_parameters() {
    let uri = "file::memory:?cache=shared";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory_file_scheme_with_slashes_fails() {
    let uri = "file://:memory:";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, uri);
    let connection_error = connect(&spec).unwrap_err();
    // Rust tools can not yet detect that this variable is used in the macro below
    let _expected_error = format!("invalid uri authority: :memory:");
    assert!(matches!(
        connection_error,
        Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
    ));
    connect(&spec).unwrap_err(); // invalid uri authority: :memory:
}

#[test]
fn uri_sqlite_explicit_relative_file_scheme() {
    // local uri tests might be able to use tempdir::TempDir::new_in.
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("file:./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_explicit_relative_no_scheme() {
    // local uri tests might be able to use tempdir::TempDir::new_in.
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

/// Show that sqlite: is not a valid scheme for SQLite.
#[test]
fn uri_sqlite_relative_with_literal_sqlite_scheme_fails() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("sqlite:{temp_relative_path}");
    let spec = ConnectionSpec {
        backend_name: "sqlite".to_string(),
        conn_str: temp_relative_uri.clone(),
    };
    connect(&spec).unwrap();
    // connect succeeded, but the filename included the scheme prefix.
    fs::remove_file(temp_relative_uri).unwrap();
}

#[test]
fn uri_sqlite_relative_no_scheme() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let spec = ConnectionSpec::try_from(&temp_relative_path).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_path);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_sqlite_scheme() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let spec = ConnectionSpec::try_from(format!("sqlite:{temp_relative_path}")).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, format!("file:{temp_relative_path}"));
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_file_scheme() {
    // local uri tests might be able to use tempdir::TempDir::new_in.
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("file:{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_parameter() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("file:{temp_relative_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_file_scheme_with_slashes_fails() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("file://{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    let connection_error = connect(&spec).unwrap_err();
    // Rust tools can not yet detect that this variable is used in the macro below.
    let _expected_error = format!("invalid uri authority: {temp_relative_path}");
    assert!(matches!(
        connection_error,
        Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
    ));
    // no file to delete
}

#[test]
fn uri_sqlite_absolute() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_relative_uri = format!("file:{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_absolute_percent_encoding() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_relative_uri = format!("file:{temp_absolute_path}?cache=private").replace('-', "%2D");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_absolute_parameter_after_slash() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_relative_uri = format!("file:{temp_absolute_path}/?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    #[cfg(target_os = "windows")]
    {
        // Windows absolute paths confuse the sqlite connection string parser.
        let connection_error = connect(&spec).unwrap_err();
        // Rust tools can not yet detect that this variable is used in the macro below
        let _expected_error = format!("unable to open database file: {temp_absolute_uri}");
        assert!(matches!(
            connection_error,
            Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        connect(&spec).unwrap();
    }
}

#[test]
fn uri_sqlite_absolute_with_slashes() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_relative_uri = format!("file://{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    #[cfg(target_os = "windows")]
    {
        // Windows absolute paths confuse the sqlite connection string parser.
        let connection_error = connect(&spec).unwrap_err();
        // Rust tools can not yet detect that this variable is used in the macro below
        let _expected_error = format!("invalid uri authority: {temp_absolute_path}");
        assert!(matches!(
            connection_error,
            Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        connect(&spec).unwrap();
    }
}

#[test]
fn uri_sqlite_absolute_with_localhost() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_relative_uri = format!("file://localhost/{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name, "sqlite".to_string());
    assert_eq!(spec.conn_str, temp_relative_uri);
    connect(&spec).unwrap();
}

// pg tests cover the connection strings described at
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNSTRING and
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNECT-HOST and
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-PARAMKEYWORDS
// TODO: These tests should create a uniquely named temporary database and connect to it.

#[test]
fn uri_pg_key_value_pair() {
    let uri = "host=localhost port=5432 dbname=mydb";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_key_value_pair_dbname_only() {
    let uri = "dbname=mydb";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_key_value_pair_dbname_only_with_space_before_equals_sign() {
    let uri = "dbname = mydb";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_key_value_pair_host_only() {
    let uri = "host=localhost";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_key_value_pair_hostaddr_only() {
    let uri = "hostaddr=1.2.3.4";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_key_value_pair_user_only() {
    let uri = "user=pguser";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgres_scheme() {
    let uri = "postgres://user:pass@localhost:1234/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme() {
    let uri = "postgresql://user:pass@localhost:1234/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme_only() {
    let uri = "postgresql://";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme_ipv6() {
    let uri = "postgresql://[2001:db8::1234]/database";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

// TODO: This may be an invalid connection string.
// See "host"
// > If the host name starts with @, it is taken as a Unix-domain socket in the abstract namespace
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-PARAMKEYWORDS
#[test]
fn uri_pg_postgresql_scheme_abstract_namespace_unix_socket() {
    let uri = "postgresql://@foo/database";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme_multi_host() {
    let uri = "postgresql://user:pass@host1:1234,host2:5678/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme_with_parameter() {
    let uri = "postgresql://user:pass@localhost:1234/dbname?connect_timeout=10";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_postgresql_scheme_with_parameter_for_host() {
    let uri = "postgresql:///dbname?host=localhost&port=1234";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_absolute_dir_postgresql_scheme_with_parameter_for_host() {
    let uri = "postgresql:///dbname?host=/var/lib/postgresql";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_pg_absolute_dir_postgresql_scheme() {
    let uri = "postgresql://%2Fvar%2Flib%2Fpostgresql/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, uri.to_string());
}

#[test]
fn uri_unsupported_scheme() {
    let spec = ConnectionSpec::try_from("other://anything").unwrap();
    assert_eq!(spec.backend_name, "other".to_string());
    assert_eq!(spec.conn_str, "other://anything".to_string());
}

#[test]
fn uri_unsupported_scheme_alt() {
    let spec = ConnectionSpec::try_from("other:anything").unwrap();
    assert_eq!(spec.backend_name, "other".to_string());
    assert_eq!(spec.conn_str, "other:anything".to_string());
}

/// Test the connection URI for PostgreSQL is accepted by the pg backend.
///
/// This test doesnt actually connect to a database, it just checks that the connection URI
/// is accepted by the pg backend and the error is the same as the error returned by the
/// connection logic for a failed connection to a "host=.. user=.." style connection string.
#[tokio::test]
async fn connect_uri_pg() {
    let uri = "postgres://user:pass@localhost:1234/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name, "pg".to_string());

    let result = connect_async(&spec).await;
    assert!(matches!(result, Err(butane_core::Error::Postgres(_))));
    match result {
        Err(butane_core::Error::Postgres(e)) => {
            assert!(format!("{e:?}").contains("Connect"));
            eprintln!("{e}");
            #[cfg(target_os = "windows")]
            assert!(format!("{e}").contains("error connecting to server"));
            #[cfg(not(target_os = "windows"))]
            assert!(format!("{e}").contains("Connection refused (os error "));
        }
        _ => panic!(),
    }
}

#[test]
fn persist_invalid_connection_backend() {
    let spec = ConnectionSpec::new("unknown_name", "foo://bar");
    assert_eq!(spec.backend_name, "unknown_name".to_string());
    assert_eq!(spec.conn_str, "foo://bar".to_string());
    let result = spec.get_backend();
    assert!(result.is_err());
    assert!(matches!(result, Err(butane_core::Error::UnknownBackend(_))));

    let dir = tempfile::TempDir::new().unwrap();
    assert!(spec.save(dir.path()).is_ok());
    let loaded_spec = ConnectionSpec::load(dir.path()).unwrap();
    assert_eq!(spec, loaded_spec);
}

#[tokio::test]
async fn invalid_pg_connection() {
    let spec = ConnectionSpec::new("pg", "does_not_parse");
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(spec.conn_str, "does_not_parse".to_string());

    let result = connect_async(&spec).await;
    assert!(matches!(result, Err(butane_core::Error::Postgres(_))));
    match result {
        Err(butane_core::Error::Postgres(e)) => {
            assert!(format!("{e:?}").contains("ConfigParse"));
            assert_eq!(format!("{e}"), "invalid connection string: unexpected EOF");
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn unreachable_pg_connection() {
    let spec = ConnectionSpec::new("pg", "host=does_not_exist user=does_not_exist");
    assert_eq!(spec.backend_name, "pg".to_string());
    assert_eq!(
        spec.conn_str,
        "host=does_not_exist user=does_not_exist".to_string()
    );

    let result = connect_async(&spec).await;
    assert!(matches!(result, Err(butane_core::Error::Postgres(_))));
    match result {
        Err(butane_core::Error::Postgres(e)) => {
            assert!(format!("{e:?}").contains("Connect"));
            eprintln!("{e}");
            #[cfg(target_os = "windows")]
            assert!(format!("{e}").contains("No such host is known"));
            #[cfg(not(target_os = "windows"))]
            assert!(format!("{e}").contains("failed to lookup address information"));
        }
        _ => panic!(),
    }
}

#[butane_test(nomigrate)]
async fn debug_connection(conn: ConnectionAsync) {
    let backend_name = conn.backend_name();

    let debug_str = format!("{conn:?}");
    if backend_name == "pg" {
        assert!(debug_str.contains("conn: true"));
    } else {
        assert!(debug_str.contains("path: Some(\"\")"));
    }
}

#[test]
fn wont_load_connection_spec_from_missing_path() {
    // prepare an non-existent path
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_owned();
    let path = std::path::Path::new(&path);
    assert!(dir.close().is_ok());
    assert!(!path.is_dir());

    // try to load a spec from the non-existent path
    let result = ConnectionSpec::load(path);
    assert!(result.is_err());
    assert!(matches!(result, Err(butane_core::Error::IO(_))));
}

#[test]
fn saves_invalid_connection_spec_to_missing_path() {
    // prepare an non-existent path
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_owned();
    let path = std::path::Path::new(&path);
    assert!(dir.close().is_ok());
    assert!(!path.is_dir());

    // writes the json to that path
    let spec = ConnectionSpec::new("unknown_name", "foo://bar");
    let result = spec.save(path);
    assert!(result.is_ok());
    let f = std::fs::File::open(path).unwrap();
    assert!(f.metadata().unwrap().is_file());

    let loaded_spec = ConnectionSpec::load(path).unwrap();
    assert_eq!(spec, loaded_spec);
}
