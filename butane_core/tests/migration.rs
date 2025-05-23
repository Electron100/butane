extern crate alloc;

use butane_core::codegen::{butane_type_with_migrations, model_with_migrations};
use butane_core::db::{BackendConnection, Connection};
use butane_core::migrations::adb::{DeferredSqlType, TypeIdentifier, TypeKey};
use butane_core::migrations::{MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut};
use butane_core::{SqlType, SqlVal};
#[cfg(feature = "pg")]
use butane_test_helper::pg_connection;
#[cfg(feature = "sqlite")]
use butane_test_helper::sqlite_connection;
use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use quote::quote;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;

#[test]
fn current_migration_basic() {
    let tokens = quote! {
        struct Foo {
            id: i64,
            bar: String,
            baz: f64,
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

    let baz_col = table.column("baz").unwrap();
    assert_eq!(baz_col.name(), "baz");
    assert!(!baz_col.nullable());
    assert!(!baz_col.is_pk());
    assert_eq!(*baz_col.default(), None);
    assert_eq!(baz_col.typeid().unwrap(), TypeIdentifier::Ty(SqlType::Real));
    assert!(!baz_col.is_auto());

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
            id: AutoPk<i64>,
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
        &mut sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 0;",
        "ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_field_pg() {
    let (mut conn, _data) = pg_connection();
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
        &mut sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 42;",
        // See comments on migration_add_field_sqlite
        "ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_field_with_default_pg() {
    let (mut conn, _data) = pg_connection();
    migration_add_field_with_default(
        &mut conn,
        "ALTER TABLE Foo ADD COLUMN baz BIGINT NOT NULL DEFAULT 42;",
        "ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_modify_field_pg() {
    env_logger::try_init().ok();
    let (mut conn, _data) = pg_connection();
    // Not verifying rename right now because we don't detect it
    // https://github.com/Electron100/butane/issues/89

    migration_modify_field_type_change(
        &mut conn,
        "ALTER TABLE Foo ALTER COLUMN bar SET DATA TYPE BIGINT;",
        "ALTER TABLE Foo ALTER COLUMN bar SET DATA TYPE INTEGER;",
    );

    migration_modify_field_nullability_change(
        &mut conn,
        "ALTER TABLE Foo ALTER COLUMN bar DROP NOT NULL;",
        "ALTER TABLE Foo ALTER COLUMN bar SET NOT NULL;",
    );

    migration_modify_field_pkey_change(
        &mut conn,
        "ALTER TABLE Foo DROP CONSTRAINT IF EXISTS Foo_pkey;\nALTER TABLE Foo ADD PRIMARY KEY (baz);",
        "ALTER TABLE Foo DROP CONSTRAINT IF EXISTS Foo_pkey;\nALTER TABLE Foo ADD PRIMARY KEY (bar);",
    );

    migration_modify_field_uniqueness_change(
        &mut conn,
        "ALTER TABLE Foo ADD UNIQUE (bar);",
        "ALTER TABLE Foo DROP CONSTRAINT Foo_bar_key;",
    );

    migration_modify_field_default_added(
        &mut conn,
        "ALTER TABLE Foo ALTER COLUMN bar SET DEFAULT 42;",
        "ALTER TABLE Foo ALTER COLUMN bar DROP DEFAULT;",
    );

    migration_modify_field_different_default(
        &mut conn,
        "ALTER TABLE Foo ALTER COLUMN bar SET DEFAULT 42;",
        "ALTER TABLE Foo ALTER COLUMN bar SET DEFAULT 41;",
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn migration_add_and_remove_field_sqlite() {
    migration_add_and_remove_field(
        &mut sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 0;ALTER TABLE Foo DROP COLUMN bar;",
        "ALTER TABLE Foo ADD COLUMN bar TEXT NOT NULL DEFAULT '';ALTER TABLE Foo DROP COLUMN baz;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_add_and_remove_field_pg() {
    let (mut conn, _data) = pg_connection();
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
        &mut sqlite_connection(),
        "DROP TABLE Foo;",
        "CREATE TABLE Foo (\"id\" INTEGER NOT NULL PRIMARY KEY,bar TEXT NOT NULL) STRICT;",
    );
}

#[cfg(feature = "pg")]
#[test]
fn migration_delete_table_pg() {
    let (mut conn, _data) = pg_connection();
    migration_delete_table(
        &mut conn,
        "DROP TABLE Foo;",
        "CREATE TABLE Foo (\"id\" BIGINT NOT NULL PRIMARY KEY,bar TEXT NOT NULL);",
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
    let backends = nonempty::nonempty![backend];
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backends, "init", None).unwrap());

    model_with_migrations(v2_tokens, &mut ms);
    assert!(ms
        .create_migration(&backends, "v2", ms.latest().as_ref())
        .unwrap());

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);

    ms.migrate(conn).unwrap();

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 0);

    verify_sql(conn, &ms, expected_up_sql, expected_down_sql);

    // Now downgrade, just to make sure we can
    ms.unmigrate(conn).unwrap();

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);
}

fn verify_sql(
    conn: &Connection,
    ms: &impl Migrations,
    expected_up_sql: &str,
    expected_down_sql: &str,
) {
    let dialect = GenericDialect {};
    let expected_up_ast = SqlParser::parse_sql(&dialect, expected_up_sql).unwrap();
    let expected_down_ast = SqlParser::parse_sql(&dialect, expected_down_sql).unwrap();

    let backend = conn.backend();
    let v2_migration = ms.latest().unwrap();

    let actual_up_sql = v2_migration.up_sql(backend.name()).unwrap().unwrap();
    let actual_up_ast = sqlparser::parser::Parser::parse_sql(&dialect, &actual_up_sql).unwrap();
    assert_eq!(actual_up_ast, expected_up_ast);
    let actual_down_sql = v2_migration.down_sql(backend.name()).unwrap().unwrap();
    let actual_down_ast = sqlparser::parser::Parser::parse_sql(&dialect, &actual_down_sql).unwrap();
    assert_eq!(actual_down_ast, expected_down_ast);
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

fn migration_modify_field_type_change(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: i32,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            bar: i64,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_modify_field_nullability_change(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: i32,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            bar: Option<i32>,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_modify_field_uniqueness_change(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: i32,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            #[unique]
            bar: i32,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_modify_field_pkey_change(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            #[pk]
            bar: i64,
            baz: i32,
        }
    };

    let v2 = quote! {
        struct Foo {
            bar: i64,
            #[pk]
            baz: i32
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_modify_field_default_added(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            #[default=42]
            bar: String,
        }
    };
    test_migrate(conn, init, v2, up_sql, down_sql);
}

fn migration_modify_field_different_default(conn: &mut Connection, up_sql: &str, down_sql: &str) {
    let init = quote! {
        struct Foo {
            id: i64,
            #[default=41]
            bar: String,
        }
    };

    let v2 = quote! {
        struct Foo {
            id: i64,
            #[default=42]
            bar: String,
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
    let backends = nonempty::nonempty![backend];
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backends, "init", None).unwrap());

    ms.current().delete_table("Foo").unwrap();
    assert!(ms
        .create_migration(&backends, "v2", ms.latest().as_ref())
        .unwrap());

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);

    ms.migrate(conn).unwrap();

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 0);

    verify_sql(conn, &ms, expected_up_sql, expected_down_sql);

    // Now downgrade, just to make sure we can
    ms.unmigrate(conn).unwrap();

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);
}
