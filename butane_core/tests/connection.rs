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
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);

    assert!(spec.connection_string_uri().is_none());

    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory() {
    let uri = ":memory:";

    // This sqlite connection string is not a valid URI.
    url::Url::parse(uri).unwrap_err();

    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);

    assert!(spec.connection_string_uri().is_none());

    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory_parameter_fails() {
    let uri = ":memory:?mode=ro";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);
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
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);
    connect(&spec).unwrap();

    let uri = spec.connection_string_uri().unwrap();
    assert_eq!(uri.scheme(), "file");
}

#[test]
fn uri_sqlite_memory_file_scheme_parameters() {
    let uri = "file::memory:?cache=shared";

    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_sqlite_memory_file_scheme_with_slashes_fails() {
    let uri = "file://:memory:";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), uri);
    let connection_error = connect(&spec).unwrap_err();
    // Rust tools can not yet detect that this variable is used in the macro below
    let _expected_error = format!("invalid uri authority: :memory:");
    assert!(matches!(
        connection_error,
        Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
    ));
}

#[test]
fn uri_sqlite_explicit_relative_file_scheme() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let temp_relative_uri = format!("file:./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_raw_filename() {
    // This doesnt use tempfile in order that there is no subdirectory prefix.
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());
    let temp_relative_uri = format!("file:./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_explicit_relative_no_scheme() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let temp_relative_uri = format!("./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);

    assert!(spec.connection_string_uri().is_none());

    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

/// Show that sqlite: is not a valid scheme for SQLite.
#[test]
fn uri_sqlite_relative_with_literal_sqlite_scheme_fails() {
    let temp_relative_path = format!("sqlite-test-{}.db", uuid::Uuid::new_v4());

    let temp_relative_uri = format!("sqlite:{temp_relative_path}");
    // Avoids ConnectionSpec::try_from as it will change sqlite: to file:.
    let spec = ConnectionSpec {
        backend_name: "sqlite".to_string(),
        conn_str: temp_relative_uri.clone(),
    };
    connect(&spec).unwrap();

    // connect succeeded, but the filename included the scheme prefix.
    #[cfg(not(target_os = "windows"))]
    {
        fs::remove_file(temp_relative_uri).unwrap();
    }
}

#[test]
fn uri_sqlite_relative_no_scheme() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    // This sqlite connection string is not a valid URI.
    url::Url::parse(&temp_relative_path).unwrap_err();

    let spec = ConnectionSpec::try_from(&temp_relative_path).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_path);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_no_scheme_with_params_doesnt_work() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db?mode=ro",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    // This sqlite connection string is not a valid URI.
    url::Url::parse(&temp_relative_path).unwrap_err();

    let spec = ConnectionSpec::try_from(&temp_relative_path).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_path);

    #[cfg(target_os = "windows")]
    {
        // Windows does not support creating files that start with a colon.
        let connection_error = connect(&spec).unwrap_err();
        // Rust tools can not yet detect that this variable is used in the macro below
        let _expected_error = format!("iunable to open database file: {temp_relative_path}");
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
        fs::remove_file(temp_relative_path).unwrap();
    }
}

#[test]
fn uri_sqlite_relative_sqlite_scheme() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let spec = ConnectionSpec::try_from(format!("sqlite:{temp_relative_path}")).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(
        spec.connection_string(),
        &format!("file:{temp_relative_path}")
    );
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_file_scheme() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let temp_relative_uri = format!("file:{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_parameter() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let temp_relative_uri = format!("file:{temp_relative_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    connect(&spec).unwrap();
    fs::remove_file(temp_relative_path).unwrap();
}

#[test]
fn uri_sqlite_relative_file_scheme_with_slashes_fails() {
    let current_directory = std::env::current_dir().unwrap();
    let temp_dir = tempfile::TempDir::new_in(&current_directory).unwrap();
    let temp_relative_path = format!(
        "{}/sqlite-test.db",
        temp_dir
            .path()
            .strip_prefix(&current_directory)
            .unwrap()
            .display()
    );
    assert!(temp_relative_path.starts_with(".tmp"));

    let temp_relative_uri = format!("file://{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    let connection_error = connect(&spec).unwrap_err();
    // Rust tools can not yet detect that this variable is used in the macro below.
    let _expected_error = format!("invalid uri authority: {temp_relative_path}");
    assert!(matches!(
        connection_error,
        Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
    ));

    let temp_relative_uri = format!("file://./{temp_relative_path}");
    let spec = ConnectionSpec::try_from(&temp_relative_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_relative_uri);
    let connection_error = connect(&spec).unwrap_err();
    // Rust tools can not yet detect that this variable is used in the macro below.
    let _expected_error = format!("invalid uri authority: {temp_relative_path}");
    assert!(matches!(
        connection_error,
        Error::SQLite(rusqlite::Error::SqliteFailure(_, Some(_expected_error)))
    ));
}

#[test]
fn uri_sqlite_absolute() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_absolute_path = format!("{}/sqlite-test.db", temp_dir.path().display());
    #[cfg(target_os = "windows")]
    assert!(temp_absolute_path.contains(":\\"));
    #[cfg(not(target_os = "windows"))]
    assert!(temp_absolute_path.starts_with("/"));
    let temp_absolute_uri = format!("file:{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_absolute_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_absolute_uri);
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
    let temp_absolute_uri = format!("file:{temp_absolute_path}?cache=private").replace('-', "%2D");
    let spec = ConnectionSpec::try_from(&temp_absolute_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_absolute_uri);
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
    let temp_absolute_uri = format!("file:{temp_absolute_path}/?cache=private");
    let spec = ConnectionSpec::try_from(&temp_absolute_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_absolute_uri);
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
    let temp_absolute_uri = format!("file://{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_absolute_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_absolute_uri);
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
    let temp_absolute_uri = format!("file://localhost/{temp_absolute_path}?cache=private");
    let spec = ConnectionSpec::try_from(&temp_absolute_uri).unwrap();
    assert_eq!(spec.backend_name(), "sqlite");
    assert_eq!(spec.connection_string(), &temp_absolute_uri);
    connect(&spec).unwrap();
}

// pg tests cover the connection strings described at
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNSTRING and
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNECT-HOST and
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-PARAMKEYWORDS

#[test]
fn pg_key_value_pairs() {
    let pairs = format!("host=/tmp user=postgres");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);

    assert!(spec.connection_string_uri().is_none());
}

#[test]
#[cfg(not(target_os = "windows"))]
fn pg_key_value_pairs_connect() {
    let pg_server = pg_tmp_server_create(PgServerOptions::default()).unwrap();
    let host = pg_server.sockdir.path().to_str().unwrap();
    assert!(pg_server.sockdir.path().exists());

    let pairs = format!("host={host} user=postgres");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    eprintln!("Connecting to {pairs}: {spec:?}");
    connect(&spec).unwrap();
}

#[test]
#[cfg(not(target_os = "windows"))]
fn pg_key_value_pairs_host_only_unix_socket() {
    let username = whoami::username();
    let pg_server = pg_tmp_server_create(PgServerOptions {
        user: Some(username.clone()),
        ..PgServerOptions::default()
    })
    .unwrap();
    let host = pg_server.sockdir.path().to_str().unwrap();

    let pairs = format!("host={host}");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    let error = connect(&spec).unwrap_err();

    eprintln!("Error: {error}");
    let expected_error =
        format!("Postgres error db error: FATAL: database \"{username}\" does not exist");
    assert!(error.to_string().contains(&expected_error));

    let pairs = format!("host = {host}");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    let error = connect(&spec).unwrap_err();
    assert!(error.to_string().contains(&expected_error));
}

#[test]
fn pg_key_value_pairs_host_only_tcpip() {
    let pairs = format!("host=localhost");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    // Same as above, this cant connect because it will attempt tp connect using the current username.
    // connect(&spec).unwrap();

    let pairs = format!("host = localhost");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    // connect(&spec).unwrap();
}

#[test]
fn pg_key_value_pairs_hostaddr_only() {
    let pairs = "hostaddr=127.0.0.1";
    let spec = ConnectionSpec::try_from(pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), pairs);
    // Same as above, this cant connect because it will attempt tp connect using the current username.
    // connect(&spec).unwrap();
}

#[test]
fn pg_key_value_pairs_port_only() {
    let pairs = "port=5432";
    let spec = ConnectionSpec::try_from(pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), pairs);
    // Not supported by the pg backend. https://github.com/sfackler/rust-postgres/issues/1239
    // https://github.com/sfackler/rust-postgres/issues/362 could be solved by finding the
    // socket directory and the socket file in the socket directory.
    // connect(&spec).unwrap();
}

#[test]
fn pg_key_value_pairs_user_only() {
    // Not supported by the pg backend. https://github.com/sfackler/rust-postgres/issues/1239
    let pairs = "user=pguser";
    let spec = ConnectionSpec::try_from(pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), pairs);
}

#[test]
fn pg_key_value_pairs_dbname_only() {
    // Not supported by the pg backend. https://github.com/sfackler/rust-postgres/issues/1239
    let pairs = "dbname=db";
    let spec = ConnectionSpec::try_from(pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), pairs);
}

#[test]
#[cfg(target_os = "linux")]
fn pg_key_value_pairs_abstract_namespace_unix_socket() {
    let pg_server = pg_tmp_server_create(PgServerOptions {
        #[cfg(not(target_os = "macos"))]
        abstract_namespace: true,
        ..PgServerOptions::default()
    })
    .unwrap();
    let host = pg_server.sockdir.path().to_str().unwrap();

    let pairs = format!("host=@{host} user=postgres");
    let spec = ConnectionSpec::try_from(&pairs).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &pairs);
    // https://github.com/sfackler/rust-postgres/issues/1240
    let err = connect(&spec).unwrap_err();
    assert!(matches!(err, butane_core::Error::Postgres(_)));
}

#[test]
#[cfg(not(target_os = "windows"))]
fn uri_pg_postgres_scheme_without_databasae() {
    let _pg_server = pg_tmp_server_create(PgServerOptions {
        port: Some(8000),
        ..PgServerOptions::default()
    })
    .unwrap();
    let uri = "postgres://postgres@localhost:8000";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
    connect(&spec).unwrap();
}

#[test]
fn uri_pg_postgresql_scheme() {
    let uri = "postgresql://user:pass@localhost:1234/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);

    let uri = spec.connection_string_uri().unwrap();
    assert_eq!(uri.scheme(), "postgresql");
}

#[test]
fn uri_pg_postgresql_scheme_only() {
    let uri = "postgresql://";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

#[test]
fn uri_pg_postgresql_scheme_ipv6() {
    let uri = "postgresql://[2001:db8::1234]/database";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

// See "host"
// > If the host name starts with @, it is taken as a Unix-domain socket in the abstract namespace
// https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-PARAMKEYWORDS
// psql allows the formats here. tokio-postgres does not yet
// due to https://github.com/sfackler/rust-postgres/issues/1240
#[test]
#[cfg(target_os = "linux")]
fn uri_pg_postgresql_scheme_abstract_namespace_unix_socket() {
    let pg_server = pg_tmp_server_create(PgServerOptions {
        abstract_namespace: true,
        ..PgServerOptions::default()
    })
    .unwrap();
    let host = pg_server.sockdir.path().to_str().unwrap();

    let uri = format!("postgresql:///?host=@{host}&user=postgres");

    let spec = ConnectionSpec::try_from(&uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &uri);

    let err = connect(&spec).unwrap_err();
    assert!(matches!(err, butane_core::Error::Postgres(_)));

    // The host part needs to be percent encoded if put into the host of the URI.
    let host = host.replace('/', "%2F");
    let uri = format!("postgresql://%40{host}/?user=postgres");

    let spec = ConnectionSpec::try_from(&uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &uri);

    let uri = format!("postgresql://postgres@%40{host}/");

    let spec = ConnectionSpec::try_from(&uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), &uri);
}

#[test]
fn uri_pg_postgresql_scheme_multi_host() {
    let uri = "postgresql://user:pass@host1:1234,host2:5678/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);

    assert!(spec.connection_string_uri().is_none());
}

#[test]
fn uri_pg_postgresql_scheme_with_parameter() {
    let uri = "postgresql://user:pass@localhost:1234/dbname?connect_timeout=10";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

#[test]
fn uri_pg_postgresql_scheme_with_parameter_for_host() {
    let uri = "postgresql:///dbname?host=localhost&port=1234";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

#[test]
fn uri_pg_absolute_dir_postgresql_scheme_with_parameter_for_host() {
    let uri = "postgresql:///dbname?host=/var/lib/postgresql";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

#[test]
fn uri_pg_absolute_dir_postgresql_scheme() {
    let uri = "postgresql://%2Fvar%2Flib%2Fpostgresql/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), uri);
}

#[test]
fn uri_unsupported_scheme() {
    let spec = ConnectionSpec::try_from("other://anything").unwrap();
    assert_eq!(spec.backend_name(), "other");
    assert_eq!(spec.connection_string(), "other://anything");
}

#[test]
fn uri_unsupported_scheme_alt() {
    let spec = ConnectionSpec::try_from("other:anything").unwrap();
    assert_eq!(spec.backend_name(), "other");
    assert_eq!(spec.connection_string(), "other:anything");
    let uri = spec.connection_string_uri().unwrap();
    assert_eq!(uri.scheme(), "other");
}

/// Test the connection URI for PostgreSQL is accepted by the pg backend.
///
/// This test doesnt actually connect to a database, it just checks that the connection URI
/// is accepted by the pg backend and the error is the same as the error returned by the
/// connection logic for a failed connection to a "host=.. user=.." style connection string.
#[tokio::test]
async fn connect_uri_pg_error() {
    let uri = "postgres://user:pass@localhost:1234/dbname";
    let spec = ConnectionSpec::try_from(uri).unwrap();
    assert_eq!(spec.backend_name(), "pg");

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
    assert_eq!(spec.backend_name(), "unknown_name");
    assert_eq!(spec.connection_string(), "foo://bar");
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
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(spec.connection_string(), "does_not_parse");

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
    assert_eq!(spec.backend_name(), "pg");
    assert_eq!(
        spec.connection_string(),
        "host=does_not_exist user=does_not_exist"
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
