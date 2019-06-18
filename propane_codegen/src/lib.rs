extern crate proc_macro;

use failure::Error;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use quote::{quote, ToTokens};
use std::collections::HashSet;
use std::path::PathBuf;
use std::result::Result;
use syn::parse_quote;
use syn::{
    Attribute, Field, FnArg, ItemStruct, LitStr, Pat, TraitItem, TraitItemMethod, Type, TypePath,
};

use propane_core::migrations;
use propane_core::*;

#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut result: TokenStream2 = input.clone().into();

    // Read the struct name and all fields
    let ast_struct: ItemStruct = syn::parse(input).unwrap();

    write_table_to_disk(&ast_struct).unwrap();

    result.into()
}

fn write_table_to_disk(ast_struct: &ItemStruct) -> Result<(), Error> {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push("propane");
    dir.push("migrations");
    migrations::from_root(&dir)
        .get_migration("current")
        .write_table(&create_atable(ast_struct))
}

fn create_atable(ast_struct: &ItemStruct) -> ATable {
    let columns: HashSet<AColumn> = ast_struct
        .fields
        .iter()
        .map(|f| AColumn {
            name: f
                .ident
                .clone()
                .expect("db object fields must be named")
                .to_string(),
            sqltype: get_sql_type(&f),
            nullable: is_nullable(&f),
            pk: is_pk(&f),
            default: get_default(&f),
        })
        .collect();

    ATable {
        name: ast_struct.ident.to_string(),
        columns,
    }
}

fn is_nullable(field: &Field) -> bool {
    let option: TypePath = parse_quote!(std::option::Option);
    match &field.ty {
        Type::Path(tp) => option == *tp,
        _ => false,
    }
}

fn is_pk(field: &Field) -> bool {
    has_attr(&field.attrs, "pk")
}

fn has_attr(attrs: &Vec<Attribute>, name: &str) -> bool {
    attrs
        .iter()
        .find(|a| match a.parse_meta() {
            Ok(m) => m.name().to_string() == name,
            _ => false,
        })
        .is_some()
}

fn get_default(field: &Field) -> Option<SqlVal> {
    // TODO support default values
    None
}

fn get_sql_type(field: &Field) -> AType {
    // Todo support Date, Tmestamp, and Blob
    if field.ty == parse_quote!(bool) {
        AType::Bool
    } else if field.ty == parse_quote!(u8)
        || field.ty == parse_quote!(i8)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i16)
        || field.ty == parse_quote!(u16)
        || field.ty == parse_quote!(i32)
    {
        AType::Int
    } else if field.ty == parse_quote!(u64) || field.ty == parse_quote!(i64) {
        AType::BigInt
    } else if field.ty == parse_quote!(f32) || field.ty == parse_quote!(f64) {
        AType::Real
    } else if field.ty == parse_quote!(String) {
        AType::Text
    } else {
        panic!(
            "Unsupported type {} for field '{}'",
            field.ty.clone().into_token_stream(),
            field.ident.clone().expect("model fields must be named")
        );
    }
}
