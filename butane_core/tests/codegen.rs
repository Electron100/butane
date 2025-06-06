use butane_core::codegen::{
    get_deferred_sql_type, get_primitive_sql_type, make_ident_literal_str, make_lit,
};
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
fn primitive_sql_type() {
    let path: syn::Path = syn::parse_quote!(i8);
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
