use butane_core::codegen::{get_deferred_sql_type, make_ident_literal_str, make_lit};
use butane_core::migrations::adb::{DeferredSqlType, TypeIdentifier, TypeKey};
use butane_core::SqlType;
use proc_macro2::Span;
use syn::{Ident, LitStr};

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
fn test_get_deferred_sql_type() {
    // primitive type
    let type_path: syn::TypePath = syn::parse_quote!(i8);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    let type_path: syn::TypePath = syn::parse_quote!(Option<i8>);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::KnownId(TypeIdentifier::Ty(sql_type)) = rv {
        assert_eq!(sql_type, SqlType::Int);
    } else {
        panic!()
    }

    // custom types
    let type_path: syn::TypePath = syn::parse_quote!(Foo);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "Foo");
    } else {
        panic!()
    }

    let type_path: syn::TypePath = syn::parse_quote!(Option<Foo>);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "Foo");
    } else {
        panic!()
    }

    // foreign keys to custom types
    let type_path: syn::TypePath = syn::parse_quote!(butane::ForeignType<Foo>);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "butane::ForeignType<Foo>");
    } else {
        panic!()
    }

    let type_path: syn::TypePath = syn::parse_quote!(Option<butane::ForeignType<Foo>>);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "butane::ForeignType<Foo>");
    } else {
        panic!()
    }

    let type_path: syn::TypePath = syn::parse_quote!(butane::Many<Foo>);
    let typ = syn::Type::Path(type_path);
    let rv = get_deferred_sql_type(&typ);
    if let DeferredSqlType::Deferred(TypeKey::CustomType(typ)) = rv {
        assert_eq!(typ, "butane::Many<Foo>");
    } else {
        panic!()
    }
}
