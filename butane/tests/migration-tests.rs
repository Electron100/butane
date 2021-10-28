use butane::migrations::{
    adb::DeferredSqlType, adb::TypeIdentifier, adb::TypeKey, MemMigrations, Migration,
    MigrationMut, Migrations, MigrationsMut,
};
use butane::{db::Connection, prelude::*, SqlType, SqlVal};
use butane_core::codegen::{butane_type_with_migrations, model_with_migrations};
use proc_macro2::TokenStream;
use quote::quote;

mod common;

#[test]
fn current_migration_basic() {
    let tokens = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    assert_eq!(db.tables().count(), 1);
    let table = db.get_table("Foo").expect("No Foo table");
    let idcol = table.column("id").unwrap();
    assert_eq!(idcol.name(), "id");
    assert!(!idcol.nullable());
    assert!(idcol.is_pk());
    assert_eq!(*idcol.default(), None);
    assert_eq!(idcol.typeid().unwrap(), TypeIdentifier::Ty(SqlType::BigInt));
    assert!(!idcol.is_auto());

    let barcol = table.column("bar").unwrap();
    assert_eq!(barcol.name(), "bar");
    assert!(!barcol.nullable());
    assert!(!barcol.is_pk());
    assert_eq!(*barcol.default(), None);
    assert_eq!(barcol.typeid().unwrap(), TypeIdentifier::Ty(SqlType::Text));
    assert!(!barcol.is_auto());

    assert_eq!(table.pk(), Some(idcol))
}

#[test]
fn current_migration_pk_attribute() {
    let tokens = quote! {
        #[derive(PartialEq, Eq, Debug, Clone)]
        struct Foo {
            #[pk]
            name: String,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    let table = db.get_table("Foo").expect("No Foo table");
    let pkcol = table.column("name").unwrap();
    assert!(pkcol.is_pk());

    assert_eq!(table.pk(), Some(pkcol))
}

#[test]
fn current_migration_default_attribute() {
    let tokens = quote! {
        #[derive(PartialEq, Eq, Debug, Clone)]
        struct Foo {
            id: i64,
            #[default="turtle"]
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    let table = db.get_table("Foo").expect("No Foo table");
    let barcol = table.column("bar").unwrap();
    assert_eq!(*barcol.default(), Some(SqlVal::Text("turtle".to_string())));
}

#[test]
fn current_migration_auto_attribute() {
    let tokens = quote! {
        #[derive(PartialEq, Eq, Debug, Clone)]
        struct Foo {
            #[auto]
            id: i64,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    let table = db.get_table("Foo").expect("No Foo table");
    let idcol = table.column("id").unwrap();
    assert!(idcol.is_auto());
}

#[test]
fn current_migration_nullable_col() {
    let tokens = quote! {
        #[derive(PartialEq, Eq, Debug, Clone)]
        struct Foo {
            id: i64,
            bar: Option<String>,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    let table = db.get_table("Foo").expect("No Foo table");
    let col = table.column("bar").unwrap();
    assert!(col.nullable());
    assert_eq!(col.typeid().unwrap(), TypeIdentifier::Ty(SqlType::Text));
}

#[test]
fn current_migration_custom_type() {
    let tokens = quote! {
        #[derive(PartialEq, Eq, Debug, Clone)]
        enum Frobnozzle {
            Foo,
            Bar,
            Baz,
        }
    };
    let mut ms = MemMigrations::new();
    butane_type_with_migrations(quote! {Text}, tokens, &mut ms);

    let tokens = quote! {
            #[derive(PartialEq, Eq, Debug, Clone)]
            struct HasCustomField {
                    id: i64,
                    frob: Frobnozzle,
            }
    };
    model_with_migrations(tokens, &mut ms);

    let m = ms.current();
    let db = m.db().unwrap();
    eprintln!("types {:?}", db.types());
    assert_eq!(
        db.types()
            .get(&TypeKey::CustomType("Frobnozzle".to_string())),
        Some(&DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)))
    );
    let table = db
        .get_table("HasCustomField")
        .expect("No HasCustomField table");
    let col = table.column("frob").expect("No frob field");
    assert_eq!(col.typeid().unwrap(), TypeIdentifier::Ty(SqlType::Text));
}

#[cfg(feature = "sqlite")]
#[test]
fn migration_add_field_sqlite() {
    migration_add_field(
        &mut common::sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 0;",
				// The exact details of futzing a DROP COLUMN in sqlite aren't
				// important (e.g. the temp table naming is certainly not part
				// of the API contract), but the goal here is to ensure we're
				// getting sane looking downgrade sql and a test failure if it
				// changes. If the change is innocuous, this test should just
				// be updated.
        "CREATE TABLE Foo__butane_tmp (id INTEGER NOT NULL PRIMARY KEY,bar TEXT NOT NULL);INSERT INTO Foo__butane_tmp SELECT id, bar FROM Foo;DROP TABLE Foo;ALTER TABLE Foo__butane_tmp RENAME TO Foo;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_field_pg() {
    let (mut conn, _data) = common::pg_connection();
    migration_add_field(
        &mut conn,
        "ALTER TABLE Foo ADD COLUMN baz BIGINT NOT NULL DEFAULT 0;",
        "ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn migration_add_field_with_default_sqlite() {
    migration_add_field_with_default(
        &mut common::sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 42;",
				// See comments on migration_add_field_sqlite
				"CREATE TABLE Foo__butane_tmp (id INTEGER NOT NULL PRIMARY KEY,bar TEXT NOT NULL);INSERT INTO Foo__butane_tmp SELECT id, bar FROM Foo;DROP TABLE Foo;ALTER TABLE Foo__butane_tmp RENAME TO Foo;"
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_field_with_default_pg() {
    let (mut conn, _data) = common::pg_connection();
    migration_add_field_with_default(
        &mut conn,
        "ALTER TABLE Foo ADD COLUMN baz BIGINT NOT NULL DEFAULT 42;",
        "ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn migration_add_and_remove_field_sqlite() {
    migration_add_and_remove_field(
        &mut common::sqlite_connection(),
				// The exact details of futzing a DROP COLUMN in sqlite aren't
				// important (e.g. the temp table naming is certainly not part
				// of the API contract), but the goal here is to ensure we're
				// getting sane looking downgrade sql and a test failure if it
				// changes. If the change is innocuous, this test should just
				// be updated.
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 0;CREATE TABLE Foo__butane_tmp (id INTEGER NOT NULL PRIMARY KEY,baz INTEGER NOT NULL);INSERT INTO Foo__butane_tmp SELECT id, baz FROM Foo;DROP TABLE Foo;ALTER TABLE Foo__butane_tmp RENAME TO Foo;",
        "ALTER TABLE Foo ADD COLUMN bar TEXT NOT NULL DEFAULT '';CREATE TABLE Foo__butane_tmp (id INTEGER NOT NULL PRIMARY KEY,bar TEXT NOT NULL);INSERT INTO Foo__butane_tmp SELECT id, bar FROM Foo;DROP TABLE Foo;ALTER TABLE Foo__butane_tmp RENAME TO Foo;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_and_remove_field_pg() {
    let (mut conn, _data) = common::pg_connection();
    migration_add_and_remove_field(
        &mut conn,
        "ALTER TABLE Foo ADD COLUMN baz BIGINT NOT NULL DEFAULT 0;ALTER TABLE Foo DROP COLUMN bar;",
        "ALTER TABLE Foo ADD COLUMN bar TEXT NOT NULL DEFAULT '';ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn migration_delete_table_sqlite() {
    migration_delete_table(
        &mut common::sqlite_connection(),
        "DROP TABLE Foo;",
        "CREATE TABLE Foo (id INTEGER NOT NULL PRIMARY KEY,bar TEXT NOT NULL);",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_delete_table_pg() {
    let (mut conn, _data) = common::pg_connection();
    migration_delete_table(
        &mut conn,
        "DROP TABLE Foo;",
        "CREATE TABLE Foo (id BIGINT NOT NULL PRIMARY KEY,bar TEXT NOT NULL);",
    );
}

fn test_migrate(
    conn: &mut Connection,
    init_tokens: TokenStream,
    v2_tokens: TokenStream,
    expected_up_sql: &str,
    expected_down_sql: &str,
) {
    let mut ms = MemMigrations::new();
    let backend = conn.backend();
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backend, "init", None).unwrap());

    model_with_migrations(v2_tokens, &mut ms);
    assert!(ms
        .create_migration(&backend, "v2", ms.latest().as_ref())
        .unwrap());

    let mut to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);
    for m in &to_apply {
        m.apply(conn).unwrap();
    }
    verify_sql(conn, &ms, expected_up_sql, expected_down_sql);

    // Now downgrade, just to make sure we can
    to_apply.reverse();
    for m in to_apply {
        m.downgrade(conn).unwrap();
    }
}

fn verify_sql(
    conn: &Connection,
    ms: &impl Migrations,
    expected_up_sql: &str,
    expected_down_sql: &str,
) {
    let backend = conn.backend();
    let v2_migration = ms.latest().unwrap();
    let strip = |s: String| s.replace("\n", "");

    let actual_up_sql = strip(v2_migration.up_sql(backend.name()).unwrap().unwrap());
    assert_eq!(actual_up_sql, expected_up_sql);
    let actual_down_sql = strip(v2_migration.down_sql(backend.name()).unwrap().unwrap());
    assert_eq!(actual_down_sql, expected_down_sql);
}

fn migration_add_field(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            bar: String,
            baz: u32,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_add_field_with_default(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            bar: String,
            #[default=42]
            baz: u32,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_add_and_remove_field(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            baz: u32,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_delete_table(conn: &mut Connection, expected_up_sql: &str, expected_down_sql: &str) {
    let init_tokens = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    let backend = conn.backend();
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backend, "init", None).unwrap());

    ms.current().delete_table("Foo").unwrap();
    assert!(ms
        .create_migration(&backend, "v2", ms.latest().as_ref())
        .unwrap());

    let mut to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);
    for m in &to_apply {
        m.apply(conn).unwrap();
    }
    verify_sql(conn, &ms, expected_up_sql, expected_down_sql);

    // Now downgrade, just to make sure we can
    to_apply.reverse();
    for m in to_apply {
        m.downgrade(conn).unwrap();
    }
}
