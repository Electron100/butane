use proc_macro2::TokenStream;
use propane::migrations::{
    adb::DeferredSqlType, adb::TypeKey, MemMigrations, Migration, Migrations, MigrationsMut,
};
use propane::{db::Connection, prelude::*, SqlType, SqlVal};
use propane_core::codegen::{model_with_migrations, propane_type_with_migrations};
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
    assert_eq!(idcol.sqltype().unwrap(), SqlType::BigInt);
    assert!(!idcol.is_auto());

    let barcol = table.column("bar").unwrap();
    assert_eq!(barcol.name(), "bar");
    assert!(!barcol.nullable());
    assert!(!barcol.is_pk());
    assert_eq!(*barcol.default(), None);
    assert_eq!(barcol.sqltype().unwrap(), SqlType::Text);
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
    assert_eq!(col.sqltype().unwrap(), SqlType::Text);
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
    propane_type_with_migrations(quote! {Text}, tokens, &mut ms);

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
        Some(&DeferredSqlType::Known(SqlType::Text))
    );
    let table = db
        .get_table("HasCustomField")
        .expect("No HasCustomField table");
    let col = table.column("frob").expect("No frob field");
    assert_eq!(col.sqltype().unwrap(), SqlType::Text);
}

#[test]
fn migration_add_field_sqlite() {
    migration_add_field(
        &mut common::sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 0;",
    );
}

#[test]
fn migration_add_field_with_default_sqlite() {
    migration_add_field_with_default(
        &mut common::sqlite_connection(),
        "ALTER TABLE Foo ADD COLUMN baz INTEGER NOT NULL DEFAULT 42;",
    );
}

fn test_migrate(
    conn: &mut Connection,
    init_tokens: TokenStream,
    v2_tokens: TokenStream,
    expected_sql: &str,
) {
    let mut ms = MemMigrations::new();
    let backend = conn.backend();
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backend, "init", None).unwrap());

    model_with_migrations(v2_tokens, &mut ms);
    assert!(ms
        .create_migration(&backend, "v2", ms.latest().as_ref())
        .unwrap());

    let to_apply = ms.unapplied_migrations(conn).unwrap();
    assert_eq!(to_apply.len(), 2);
    for m in to_apply {
        m.apply(conn).unwrap();
    }
    let actual_sql = ms
        .latest()
        .unwrap()
        .up_sql(backend.name())
        .unwrap()
        .unwrap();
    assert_eq!(actual_sql, expected_sql);
}

fn migration_add_field(conn: &mut Connection, sql: &str) {
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
    test_migrate(conn, init, v2, sql);
}

fn migration_add_field_with_default(conn: &mut Connection, sql: &str) {
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
    test_migrate(conn, init, v2, sql);
}
