use butane_core::codegen::{
    get_deferred_sql_type, get_primitive_sql_type, make_ident_literal_str, make_lit,
    model_with_migrations,
};
use butane_core::migrations::adb::{DeferredSqlType, TypeIdentifier, TypeKey, MANY_SUFFIX};
use butane_core::migrations::{MemMigrations, Migration, MigrationsMut};
use butane_core::SqlType;
use proc_macro2::Span;
use quote::ToTokens;
use syn::{parse_quote, Ident, LitStr};

#[test]
fn make_litstr_from_ident() {
    let result = make_ident_literal_str(&Ident::new("foo", Span::call_site()));

    assert_eq!(result, LitStr::new("foo", Span::call_site()));
}

#[test]
fn make_litstr_from_str() {
    let result = make_lit("foo");

    assert_eq!(result, LitStr::new("foo", Span::call_site()));
}

#[test]
fn primitive_sql_type() {
    let path: syn::Path = syn::parse_quote!(i8);
    let rv = get_primitive_sql_type(&path).unwrap();
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(r#i8);
    let rv = get_primitive_sql_type(&path).unwrap();
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(Option<i8>);
    assert!(get_primitive_sql_type(&path).is_none());
}

#[test]
fn deferred_sql_type_primitive() {
    let path: syn::Path = syn::parse_quote!(i8);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(r#i8);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(r#i8);
    let rv = get_primitive_sql_type(&path).unwrap();
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(Option<i8>);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }
}

#[test]
fn deferred_sql_type_user_defined_types() {
    let path: syn::Path = syn::parse_quote!(Foo);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "Foo");
    } else {
        panic!()
    }

    let path: syn::Path = syn::parse_quote!(Option<Foo>);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "Foo");
    } else {
        panic!()
    }
}

#[test]
fn deferred_sql_type_fkey() {
    let path: syn::Path = syn::parse_quote!(butane::ForeignKey<Foo>);
    let rv = get_deferred_sql_type(&path);
    assert_eq!(
        rv,
        DeferredSqlType::Deferred(TypeKey::PK("Foo".to_string()))
    );

    let path: syn::Path = syn::parse_quote!(Option<butane::ForeignKey<Foo>>);
    let rv = get_deferred_sql_type(&path);
    assert_eq!(
        rv,
        DeferredSqlType::Deferred(TypeKey::PK("Foo".to_string()))
    );

    let path: syn::Path = syn::parse_quote!(butane::Many<Foo>);
    let rv = get_deferred_sql_type(&path);
    assert_eq!(
        rv,
        DeferredSqlType::Deferred(TypeKey::CustomType("butane::Many<Foo>".into()))
    );
}

#[test]
fn deferred_sql_type_many() {
    let path: syn::Path = syn::parse_quote!(butane::Many<Foo>);
    let rv = get_deferred_sql_type(&path);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "butane::Many<Foo>");
    } else {
        panic!()
    }
}

#[test]
fn r_hash_struct_member() {
    let mut migrations = MemMigrations::default();

    let item: syn::ItemStruct = parse_quote! {
        pub struct Dummy {
            id: u32,
            r#type: String,
        }
    };

    let tokens = item.to_token_stream();
    let _model = model_with_migrations(tokens, &mut migrations);
    let migration = migrations.current();
    let adb = migration.db().unwrap();
    let table = adb.get_table("Dummy").expect("Table Dummy should exist");
    assert_eq!(table.columns[0].name(), "id");
    assert_eq!(table.columns[1].name(), "type");
}

#[test]
fn r_hash_struct_member_many() {
    let mut migrations = MemMigrations::default();

    let many_item: syn::ItemStruct = parse_quote! {
        pub struct Tag {
            #[pk]
            pub tag: String,
        }
    };

    let tokens = many_item.to_token_stream();
    let _model = model_with_migrations(tokens, &mut migrations);

    let item: syn::ItemStruct = parse_quote! {
        pub struct Dummy {
            id: u32,
            r#type: Many<Tag>,
        }
    };

    let tokens = item.to_token_stream();
    let _model = model_with_migrations(tokens, &mut migrations);
    let migration = migrations.current();
    let adb = migration.db().unwrap();
    eprintln!("ADB: {adb:?}");
    let table = adb.get_table("Dummy").expect("Table Dummy should exist");
    assert_eq!(table.columns[0].name(), "id");
    assert_eq!(table.columns.len(), 1);

    let many_table = adb
        .get_table(&format!("Dummy_type{MANY_SUFFIX}"))
        .expect("Table Dummy_type should exist");
    assert_eq!(many_table.columns.len(), 2);
    assert_eq!(many_table.columns[0].name(), "owner");
    assert_eq!(many_table.columns[1].name(), "has");
}

#[test]
fn r_hash_struct_name() {
    let mut migrations = MemMigrations::default();

    let item: syn::ItemStruct = parse_quote! {
        pub struct r#type {
            id: u32,
            foo: String,
        }
    };

    let tokens = item.to_token_stream();
    let _model = model_with_migrations(tokens, &mut migrations);
    let migration = migrations.current();
    let adb = migration.db().unwrap();
    eprintln!("ADB: {adb:?}");
    let table = adb.get_table("type").expect("Table type should exist");
    assert_eq!(table.columns[0].name(), "id");
    assert_eq!(table.columns[1].name(), "foo");

    let fkey_item: syn::ItemStruct = parse_quote! {
        pub struct fkey {
            id: u32,
            foo: ForeignKey<r#type>,
        }
    };

    let tokens = fkey_item.to_token_stream();
    let _model = model_with_migrations(tokens, &mut migrations);
    let migration = migrations.current();
    let adb = migration.db().unwrap();
    eprintln!("ADB: {adb:?}");
    let table = adb.get_table("type").expect("Table type should exist");
    assert_eq!(table.columns[0].name(), "id");
    assert_eq!(table.columns[1].name(), "foo");
}
