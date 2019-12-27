//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use proc_macro_hack::proc_macro_hack;
use propane_core::migrations::adb::{DeferredSqlType, TypeKey};
use propane_core::*;
use quote::{quote, ToTokens};
use syn;
use syn::parse_quote;
use syn::{
    Attribute, Expr, Field, ItemStruct, ItemType, Lit, LitStr, Meta, MetaNameValue, NestedMeta,
};

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
/// 1. The type of each field must implement [`FieldType`] or be [`Many`].
/// 2. There must be a primary key field. This must be either annotated with a `#[pk]` attribute or named `id`.
///
/// [`FieldType`]: crate::FieldType
/// [`Many`]: crate::Many
#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Transform into a derive because derives can have helper
    // attributes but proc macro attributes can't yet (nor can they
    // create field attributes)
    let mut ast_struct: ItemStruct = syn::parse(input).unwrap();
    let mut config = dbobj::Config::default();
    for attr in &ast_struct.attrs {
        match attr.parse_meta() {
            Ok(Meta::NameValue(MetaNameValue {
                path,
                eq_token: _,
                lit: Lit::Str(s),
            })) => {
                if path.is_ident("table") {
                    config.table_name = Some(s.value())
                }
            }
            _ => (),
        }
    }
    // Filter out our helper attributes
    let attrs: Vec<Attribute> = ast_struct
        .attrs
        .clone()
        .into_iter()
        .filter(|a| !a.path.is_ident("table"))
        .collect();

    let state_attrs = if has_derive_serialize(&attrs) {
        quote!(#[serde(skip)])
    } else {
        TokenStream2::new()
    };

    let vis = &ast_struct.vis;

    migration::write_table_to_disk(&ast_struct, &config).unwrap();

    let impltraits = dbobj::impl_dbobject(&ast_struct, &config);
    let fieldexprs = dbobj::add_fieldexprs(&ast_struct);

    match &mut ast_struct.fields {
        syn::Fields::Named(fields) => {
            for field in &mut fields.named {
                field.attrs.retain(|a| {
                    !a.path.is_ident("pk")
                        && !a.path.is_ident("auto")
                        && !a.path.is_ident("sqltype")
                        && !a.path.is_ident("default")
                });
            }
        }
        _ => panic!("Fields must be named"),
    };
    let fields = match ast_struct.fields {
        syn::Fields::Named(fields) => fields.named,
        _ => panic!("Fields must be named"),
    };

    let ident = ast_struct.ident;

    quote!(
        #(#attrs)*
        #vis struct #ident {
            #state_attrs
            state: propane::ObjectState,
            #fields
        }
        #impltraits
        #fieldexprs
    )
    .into()
}

fn has_derive_serialize(attrs: &Vec<Attribute>) -> bool {
    for attr in attrs {
        if let Ok(Meta::List(ml)) = attr.parse_meta() {
            if ml.path.is_ident("derive")
                && ml
                    .nested
                    .iter()
                    .find(|nm| match nm {
                        NestedMeta::Meta(Meta::Path(path)) => path.is_ident("Serialize"),
                        _ => false,
                    })
                    .is_some()
            {
                return true;
            }
        }
    }
    false
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

#[proc_macro_attribute]
pub fn propane_type(_args: TokenStream, input: TokenStream) -> TokenStream {
    let type_alias: syn::Result<ItemType> = syn::parse(input.clone());
    if let Ok(type_alias) = type_alias {
        if let Err(e) = migration::add_typedef(&type_alias.ident, &type_alias.ty) {
            eprintln!("unable to save typedef {}", e);
            panic!("unable to save typedef")
        } else {
            input
        }
    } else {
        quote!(compile_error!(
            "The #[propane] macro wasn't expected to be used here"
        ))
        .into()
    }
}

fn tokens_for_sqltype(ty: SqlType) -> TokenStream2 {
    match ty {
        SqlType::Bool => quote!(propane::SqlType::Bool),
        SqlType::Int => quote!(propane::SqlType::Int),
        SqlType::BigInt => quote!(propane::SqlType::BigInt),
        SqlType::Real => quote!(propane::SqlType::Real),
        SqlType::Text => quote!(propane::SqlType::Text),
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
fn get_primitive_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    // Todo support Date, Tmestamp, and Blob
    if *ty == parse_quote!(bool) {
        Some(DeferredSqlType::Known(SqlType::Bool))
    } else if *ty == parse_quote!(u8)
        || *ty == parse_quote!(i8)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i16)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i32)
    {
        Some(DeferredSqlType::Known(SqlType::Int))
    } else if *ty == parse_quote!(u32) || *ty == parse_quote!(i64) {
        // TODO better support unsigned integers here. Sqlite has no u64, though Postgres does
        Some(DeferredSqlType::Known(SqlType::BigInt))
    } else if *ty == parse_quote!(f32) || *ty == parse_quote!(f64) {
        Some(DeferredSqlType::Known(SqlType::Real))
    } else if *ty == parse_quote!(String) {
        Some(DeferredSqlType::Known(SqlType::Text))
    } else if *ty == parse_quote!(Vec<u8>) {
        Some(DeferredSqlType::Known(SqlType::Blob))
    } else if *ty == parse_quote!(NaiveDateTime) {
        Some(DeferredSqlType::Known(SqlType::Timestamp))
    } else {
        None
    }
}

fn get_option_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    get_foreign_type_argument(ty, "Option").map(|path| {
        let inner_ty: syn::Type = syn::TypePath {
            qself: None,
            path: path.clone(),
        }
        .into();

        get_deferred_sql_type(&inner_ty)
    })
}

fn get_many_sql_type(field: &Field) -> Option<DeferredSqlType> {
    get_foreign_sql_type(&field.ty, "Many")
}

fn is_many_to_many(field: &Field) -> bool {
    get_many_sql_type(field).is_some()
}

fn is_option(field: &Field) -> bool {
    get_foreign_type_argument(&field.ty, "Option").is_some()
}

/// Check for special fields which won't correspond to rows and don't
/// implement FieldType
fn is_row_field(f: &Field) -> bool {
    !is_many_to_many(f)
}

fn get_foreign_type_argument<'a>(ty: &'a syn::Type, tyname: &'static str) -> Option<&'a syn::Path> {
    let path = match ty {
        syn::Type::Path(path) => &path.path,
        _ => return None,
    };
    let seg = if path.segments.len() == 2 && path.segments.first().unwrap().ident == "propane" {
        path.segments.last()
    } else {
        path.segments.first()
    }?;
    if seg.ident != tyname {
        return None;
    }
    let args = match &seg.arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return None,
    };
    if args.len() != 1 {
        panic!("{} should have a single type argument", tyname)
    }
    match args.last().unwrap() {
        syn::GenericArgument::Type(syn::Type::Path(typath)) => Some(&typath.path),
        _ => panic!("{} argument should be a type.", tyname),
    }
}

fn get_foreign_sql_type(ty: &syn::Type, tyname: &'static str) -> Option<DeferredSqlType> {
    let typath = get_foreign_type_argument(ty, tyname);
    typath.map(|typath| {
        DeferredSqlType::Deferred(TypeKey::PK(
            typath
                .segments
                .last()
                .expect(&format!("{} must have an argument", tyname))
                .ident
                .to_string(),
        ))
    })
}

fn get_deferred_sql_type(ty: &syn::Type) -> DeferredSqlType {
    get_primitive_sql_type(ty)
        .or(get_option_sql_type(ty))
        .or(get_foreign_sql_type(ty, "ForeignKey"))
        .unwrap_or(DeferredSqlType::Deferred(TypeKey::CustomType(
            ty.clone().into_token_stream().to_string(),
        )))
}

fn pk_field(ast_struct: &ItemStruct) -> Option<Field> {
    let pk_by_attribute = fields(ast_struct).find(|f| {
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

fn is_auto(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .find(|attr| attr.path.is_ident("auto"))
        .is_some()
}

/// Defaults are used for fields added by later migrations
/// Example
/// #[default = 42]
fn get_default(field: &Field) -> Option<SqlVal> {
    // TODO these panics should report proper compiler errors
    field
        .attrs
        .iter()
        .find(|attr| attr.path.is_ident("auto"))
        .map(|attr| match attr.parse_meta() {
            Ok(Meta::NameValue(meta)) => sqlval_from_lit(meta.lit),
            _ => panic!("malformed default value"),
        })
}

fn fields(ast_struct: &ItemStruct) -> impl Iterator<Item = &Field> {
    ast_struct
        .fields
        .iter()
        .filter(|f| f.ident.clone().unwrap().to_string() != "state")
}

fn sqlval_from_lit(lit: Lit) -> SqlVal {
    // TODO these panics should report proper compiler errors
    match lit {
        Lit::Str(lit) => SqlVal::Text(lit.value()),
        Lit::ByteStr(lit) => SqlVal::Blob(lit.value()),
        Lit::Byte(lit) => panic!("single byte literal is not supported"),
        Lit::Char(lit) => panic!("single char literal is not supported"),
        Lit::Int(lit) => SqlVal::Int(lit.base10_parse().unwrap()),
        Lit::Float(lit) => SqlVal::Real(lit.base10_parse().unwrap()),
        Lit::Bool(lit) => SqlVal::Bool(lit.value),
        Lit::Verbatim(lit) => panic!("raw verbatim literals are not supported"),
    }
}
