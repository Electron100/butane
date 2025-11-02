extern crate alloc;

use butane_core::codegen::{butane_type_with_migrations, model_with_migrations};
use butane_core::db::{BackendConnectionAsync, ConnectionAsync};
use butane_core::migrations::adb::{DeferredSqlType, TypeIdentifier, TypeKey};
use butane_core::migrations::{MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut};
use butane_core::{SqlType, SqlVal};
use butane_test_helper::get_async_connection;
use butane_test_macros::butane_backend_name_test;
use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use quote::quote;

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

async fn test_migrate(
    conn: &mut ConnectionAsync,
    init_tokens: TokenStream,
    v2_tokens: TokenStream,
    test_name: &str,
) {
    let mut ms = MemMigrations::new();
    let backend = conn.backend();
    let backend_name = backend.name();
    let backends = nonempty::nonempty![backend];
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backends, "init", None).unwrap());

    model_with_migrations(v2_tokens, &mut ms);
    assert!(ms
        .create_migration(&backends, "v2", ms.latest().as_ref())
        .unwrap());

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 2);
        Ok(())
    })
    .await
    .unwrap();

    // For turso, certain migrations fail due to turso-specific bugs with ALTER TABLE
    // For tests that add/remove fields, turso panics with "table being renamed should be in schema"
    if backend_name == "turso" && test_name.starts_with("add") {
        let result = ms.migrate_async(conn).await;
        assert!(
            result.is_err(),
            "Expected turso migration to fail with 'table being renamed should be in schema' error"
        );
        return;
    }

    ms.migrate_async(conn).await.unwrap();

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 0);
        Ok(())
    })
    .await
    .unwrap();

    verify_sql(conn, &ms, test_name);

    // Now downgrade, just to make sure we can
    // Skip unmigrate for sqlite pkey_change test due to known issue
    if test_name == "modify_field_pkey_change"
        && (backend_name == "sqlite" || backend_name == "turso")
    {
        return;
    }

    ms.unmigrate_async(conn).await.unwrap();

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 2);
        Ok(())
    })
    .await
    .unwrap();
}

fn verify_sql(conn: &ConnectionAsync, ms: &impl Migrations, test_name: &str) {
    let backend = conn.backend();
    let v2_migration = ms.latest().unwrap();

    let actual_up_sql = v2_migration.up_sql(backend.name()).unwrap().unwrap();
    let up_expectation_file = format!(
        "tests/expectations/migration_{}_{}_up.sql",
        test_name,
        backend.name()
    );
    expectorate::assert_contents(&up_expectation_file, &actual_up_sql);

    let actual_down_sql = v2_migration.down_sql(backend.name()).unwrap().unwrap();
    let down_expectation_file = format!(
        "tests/expectations/migration_{}_{}_down.sql",
        test_name,
        backend.name()
    );
    expectorate::assert_contents(&down_expectation_file, &actual_down_sql);
}

#[butane_backend_name_test(async)]
async fn migration_add_field(backend_name: &str) {
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "add_field").await;
}

#[butane_backend_name_test(async)]
async fn migration_add_field_with_default(backend_name: &str) {
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "add_field_with_default").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_type_change(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_type_change").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_nullability_change(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_nullability_change").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_uniqueness_change(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_uniqueness_change").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_pkey_change(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_pkey_change").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_default_added(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_default_added").await;
}

#[butane_backend_name_test(async)]
async fn migration_modify_field_different_default(backend_name: &str) {
    env_logger::try_init().ok();
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "modify_field_different_default").await;
}

#[butane_backend_name_test(async)]
async fn migration_add_and_remove_field(backend_name: &str) {
    let mut conn = get_async_connection(backend_name).await;
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
    test_migrate(&mut conn, init, v2, "add_and_remove_field").await;
}

#[butane_backend_name_test(async)]
async fn migration_delete_table(backend_name: &str) {
    let mut conn = get_async_connection(backend_name).await;
    let init_tokens = quote! {
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    let backend = conn.backend();
    let backend_name = backend.name();
    let backends = nonempty::nonempty![backend];
    model_with_migrations(init_tokens, &mut ms);
    assert!(ms.create_migration(&backends, "init", None).unwrap());

    ms.current().delete_table("Foo").unwrap();
    assert!(ms
        .create_migration(&backends, "v2", ms.latest().as_ref())
        .unwrap());

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 2);
        Ok(())
    })
    .await
    .unwrap();

    ms.migrate_async(&mut conn).await.unwrap();

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 0);
        Ok(())
    })
    .await
    .unwrap();

    verify_sql(&conn, &ms, "delete_table");

    // Now downgrade, just to make sure we can
    // Skip unmigrate for sqlite and turso delete_table test due to known issue
    if backend_name == "sqlite" || backend_name == "turso" {
        return;
    }

    ms.unmigrate_async(&mut conn).await.unwrap();

    let ms2 = ms.clone();
    conn.with_sync(move |conn| {
        let to_apply = ms2.unapplied_migrations(conn).unwrap();
        assert_eq!(to_apply.len(), 2);
        Ok(())
    })
    .await
    .unwrap();
}
