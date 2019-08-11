//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use failure::Error;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use proc_macro_hack::proc_macro_hack;
use propane_core::migrations::adb::{DeferredSqlType, TypeKey};
use propane_core::*;
use quote::{quote, ToTokens};
use syn;
use syn::parse_quote;
use syn::{Expr, Field, ItemStruct, LitStr};

#[macro_use]
macro_rules! make_compile_error {
    ($span:expr=> $($arg:tt)*) => ({
        let lit = make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote_spanned!($span=> compile_error!(#lit))
    });
    ($($arg:tt)*) => ({
        let lit = make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote!(compile_error!(#lit))
    })
}

mod dbobj;
mod filter;
mod migration;

/// Attribute macro which marks a struct as being a data model and
/// generates an implementation of [DataObject][crate::DataObject]. This
/// macro will also write information to disk at compile time necessary to
/// generate migrations
///
/// There are a few restrictions on model types:
/// 1. The type of each field must implement [`FieldType`].
/// 2. There must be a primary key field. This must be either annotated with a `#[pk]` attribute or named `id`.
///
/// [`FieldType`]: crate::FieldType
#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Transform into a derive because derives can have helper
    // attributes but proc macro attributes can't yet (nor can they
    // create field attributes)
    let ast_struct: ItemStruct = syn::parse(input).unwrap();
    quote!(
        #[derive(propane::prelude::Model)]
        #ast_struct
    )
    .into()
}

/// Helper for the `model` macro necessary because attribute macros
/// are not allowed their own helper attributes, whereas derives are.
#[proc_macro_derive(Model, attributes(pk))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let mut result: TokenStream2 = TokenStream2::new();

    // Read the struct name and all fields
    let ast_struct: ItemStruct = syn::parse(input).unwrap();

    migration::write_table_to_disk(&ast_struct).unwrap();

    result.extend(dbobj::impl_dbobject(&ast_struct));
    result.extend(dbobj::add_fieldexprs(&ast_struct));

    result.into()
}

#[proc_macro_hack]
pub fn filter(input: TokenStream) -> TokenStream {
    let input: TokenStream2 = input.into();
    let args: Vec<TokenTree> = input.into_iter().collect();
    if args.len() < 2 {
        return quote!(compile_error!("Expected filter!(Type, expression)")).into();
    }
    let tyid = match &args[0] {
        TokenTree::Ident(tyid) => tyid.clone(),
        _ => return quote!(compile_error!("Unexpected tokens in database object type")).into(),
    };

    if let TokenTree::Punct(_) = args[1] {
    } else {
        return quote!(compile_error!("Expected filter!(Type, expression)")).into();
    }

    let expr: TokenStream2 = args.into_iter().skip(2).collect();
    let expr: Expr = syn::parse2(expr).expect("Expected filter!(Type, expression)");
    filter::for_expr(&tyid, &expr).into()
}

fn tokens_for_sqltype(ty: SqlType) -> TokenStream2 {
    match ty {
        SqlType::Bool => quote!(propane::SqlType::Bool),
        SqlType::Int => quote!(propane::SqlType::Int),
        SqlType::BigInt => quote!(propane::SqlType::BigInt),
        SqlType::Real => quote!(propane::SqlType::Real),
        SqlType::Text => quote!(propane::SqlType::Text),
        SqlType::Date => quote!(propane::SqlType::Date),
        SqlType::Timestamp => quote!(propane::SqlType::Timestamp),
        SqlType::Blob => quote!(propane::SqlType::Blob),
    }
}

fn make_ident_literal_str(ident: &Ident) -> LitStr {
    let as_str = format!("{}", ident);
    LitStr::new(&as_str, Span::call_site())
}

fn make_lit(s: &str) -> LitStr {
    LitStr::new(s, Span::call_site())
}

/// If the field refers to a primitive, return its SqlType
fn get_primitive_sql_type(field: &Field) -> Option<DeferredSqlType> {
    // Todo support Date, Tmestamp, and Blob
    if field.ty == parse_quote!(bool) {
        Some(DeferredSqlType::Known(SqlType::Bool))
    } else if field.ty == parse_quote!(u8)
        || field.ty == parse_quote!(i8)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i16)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i32)
    {
        Some(DeferredSqlType::Known(SqlType::Int))
    } else if field.ty == parse_quote!(u32) || field.ty == parse_quote!(i64) {
        // TODO better support unsigned integers here. Sqlite has no u64, though Postgres does
        Some(DeferredSqlType::Known(SqlType::BigInt))
    } else if field.ty == parse_quote!(f32) || field.ty == parse_quote!(f64) {
        Some(DeferredSqlType::Known(SqlType::Real))
    } else if field.ty == parse_quote!(String) {
        Some(DeferredSqlType::Known(SqlType::Text))
    } else {
        None
    }
}

fn get_foreign_key_sql_type(field: &Field) -> Option<DeferredSqlType> {
    let path = match &field.ty {
        syn::Type::Path(path) => &path.path,
        _ => return None,
    };
    let seg =
        if path.segments.len() == 2 && path.segments.first().unwrap().value().ident == "propane" {
            path.segments.last()
        } else {
            path.segments.first()
        }?;
    if seg.value().ident != "ForeignKey" {
        return None;
    }
    let args = match &seg.value().arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return None,
    };
    if args.len() != 1 {
        panic!("ForeignKey should have a single type argument")
    }
    let typath = match args.last().unwrap().value() {
        syn::GenericArgument::Type(syn::Type::Path(typath)) => &typath.path,
        _ => panic!("ForeignKey argument should be a type."),
    };
    Some(DeferredSqlType::Deferred(TypeKey::PK(
        typath
            .segments
            .last()
            .expect("ForeignKey must have an argument")
            .value()
            .ident
            .to_string(),
    )))
}

fn get_deferred_sql_type(field: &Field) -> DeferredSqlType {
    get_primitive_sql_type(field)
        .or(get_foreign_key_sql_type(field))
        .expect(&format!(
            "Unsupported type {} for field '{}'",
            field.ty.clone().into_token_stream(),
            field.ident.clone().expect("model fields must be named")
        ))
}

fn pk_field(ast_struct: &ItemStruct) -> Option<Field> {
    let pk_by_attribute = ast_struct.fields.iter().find(|f| {
        f.attrs
            .iter()
            .find(|attr| attr.path.is_ident("pk"))
            .is_some()
    });
    if let Some(id_field) = pk_by_attribute {
        return Some(id_field.clone());
    }
    let pk_by_name = ast_struct.fields.iter().find(|f| match &f.ident {
        Some(ident) => "id" == ident.to_string(),
        None => false,
    });
    if let Some(id_field) = pk_by_name {
        Some(id_field.clone())
    } else {
        None
    }
}
