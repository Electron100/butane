use butane_core::db::{connect, BackendConnection, Connection, ConnectionSpec};
use butane_test_helper::*;

async fn connection_not_closed(conn: Connection) {
    assert!(!conn.is_closed());
}
testall_no_migrate!(connection_not_closed);

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

    let result = connect(&spec).await;
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

    let result = connect(&spec).await;
    assert!(matches!(result, Err(butane_core::Error::Postgres(_))));
    match result {
        Err(butane_core::Error::Postgres(e)) => {
            assert!(format!("{e:?}").contains("Connect"));
            #[cfg(target_os = "windows")]
            assert!(format!("{e}").contains("No such host is known"));
            #[cfg(not(target_os = "windows"))]
            assert!(format!("{e}").contains("failed to lookup address information"));
        }
        _ => panic!(),
    }
}

async fn debug_connection(conn: Connection) {
    let backend_name = conn.backend_name();

    let debug_str = format!("{:?}", conn);
    if backend_name == "pg" {
        assert!(debug_str.contains("conn: true"));
    } else {
        assert!(debug_str.contains("path: Some(\"\")"));
    }
}
testall_no_migrate!(debug_connection);

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
