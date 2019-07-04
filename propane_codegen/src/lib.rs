//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use failure::Error;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use proc_macro_hack::proc_macro_hack;
use propane_core::*;
use quote::{quote, ToTokens};
use syn;
use syn::parse_quote;
use syn::{Expr, Field, ItemStruct, LitStr};

mod dbobj;
mod filter;
mod migration;

#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Transform into a derive because derives can have helper
    // attributes but proc macro attributes can't yet (nor can they
    // create field attributes)
    let ast_struct: ItemStruct = syn::parse(input).unwrap();
    quote!(
        #[derive(Model)]
        #ast_struct
    )
    .into()
}

#[proc_macro_derive(Model, attributes(pk))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let mut result: TokenStream2 = TokenStream2::new();

    // Read the struct name and all fields
    let ast_struct: ItemStruct = syn::parse(input).unwrap();

    migration::write_table_to_disk(&ast_struct).unwrap();

    result.extend(dbobj::add_fieldexprs_to_impl(&ast_struct));
    result.extend(dbobj::impl_dbobject(&ast_struct));

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

fn get_sql_type(field: &Field) -> SqlType {
    // Todo support Date, Tmestamp, and Blob
    if field.ty == parse_quote!(bool) {
        SqlType::Bool
    } else if field.ty == parse_quote!(u8)
        || field.ty == parse_quote!(i8)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i16)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i32)
    {
        SqlType::Int
    } else if field.ty == parse_quote!(u64) || field.ty == parse_quote!(i64) {
        SqlType::BigInt
    } else if field.ty == parse_quote!(f32) || field.ty == parse_quote!(f64) {
        SqlType::Real
    } else if field.ty == parse_quote!(String) {
        SqlType::Text
    } else {
        panic!(
            "Unsupported type {} for field '{}'",
            field.ty.clone().into_token_stream(),
            field.ident.clone().expect("model fields must be named")
        );
    }
}

fn get_deferred_sql_type(field: &Field) -> DeferredSqlType {
    DeferredSqlType::Known(get_sql_type(field))
}
