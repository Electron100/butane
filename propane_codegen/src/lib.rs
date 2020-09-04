//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, TokenTree};
use proc_macro_hack::proc_macro_hack;
use propane_core::codegen::get_deferred_sql_type;
use propane_core::migrations::adb::{DeferredSqlType, TypeKey};
use propane_core::migrations::{MigrationMut, MigrationsMut};
use propane_core::*;
use quote::quote;
use std::path::PathBuf;
use syn::{Expr, ItemEnum, ItemStruct, ItemType};

mod filter;

/// Attribute macro which marks a struct as being a data model and
/// generates an implementation of [DataObject][crate::DataObject]. This
/// macro will also write information to disk at compile time necessary to
/// generate migrations
///
/// ## Restrictions on model types:
/// 1. The type of each field must implement [`FieldType`] or be [`Many`].
/// 2. There must be a primary key field. This must be either annotated with a `#[pk]` attribute or named `id`.
///
/// ## Helper Attributes
/// * `#[table = "NAME"]` used on the struct to specify the name of the table (defaults to struct name)
/// * `#[pk]` on a field to specify that it is the primary key.
/// * `#[auto]` on a field indicates that the field's value is
///    initialized based on serial/autoincrement. Currently supported
///    only on the primary key and only if the primary key is an integer
///    type
/// * `[default]` should be used on fields added by later migrations to avoid errors on existing objects.
///     Unnecessary if the new field is an `Option<>`
///
/// For example
/// ```ignore
/// #[model]
/// #[table = "posts"]
/// pub struct Post {
///   #[auto]
///   #[pk] // unnecessary if identifier were named id instead
///   pub identifier: i32,
///   pub title: String,
///   pub content: String,
///   #[default = false]
///   pub published: bool,
/// }
/// ```
///
///
/// [`FieldType`]: crate::FieldType
/// [`Many`]: propane_core::many::Many
#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    codegen::model_with_migrations(input.into(), &mut migrations_for_dir()).into()
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

struct CustomTypeInfo {
    name: String,
    ty: DeferredSqlType,
}

/// Attribute macro which marks a type as being available to propane
/// for use in models.
///
/// May be used on type aliases, structs, or enums. Except when used
/// on type aliases, it must be given a parameter specifying the
/// SqlType it can be converted to.
///
/// E.g.
/// ```ignore
/// #[propane_type]
/// pub type CurrencyAmount = f64;
///
/// #[propane_type(Text)]
/// pub enum Currency {
///   Dollars,
///   Pounds,
///   Euros,
/// }
/// impl ToSql for Currency {
///   fn to_sql(&self) -> SqlVal {
///      SqlVal::Text(
///          match self {
///              Self::Dollars => "dollars",
///              Self::Pounds => "pounds",
///              Self::Euros => "euros",
///          }
///          .to_string())
///  }
/// }
/// ```
#[proc_macro_attribute]
pub fn propane_type(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut tyinfo: Option<CustomTypeInfo> = None;
    let type_alias: syn::Result<ItemType> = syn::parse(input.clone());
    if let Ok(type_alias) = type_alias {
        tyinfo = Some(CustomTypeInfo {
            name: type_alias.ident.to_string(),
            ty: get_deferred_sql_type(&type_alias.ty),
        })
    }

    if tyinfo.is_none() {
        // For types below here, we need the SqlType given to us
        let args: TokenStream2 = args.into();
        let args: Vec<TokenTree> = args.into_iter().collect();
        if args.len() != 1 {
            return quote!(compile_error!("Expected propane_type(sqltype)");).into();
        }
        let tyid = match &args[0] {
            TokenTree::Ident(tyid) => tyid.clone(),
            _ => return quote!(compile_error!("Unexpected tokens in propane_type");).into(),
        };
        let sqltype = match sqltype_from_name(&tyid) {
            Some(ty) => ty,
            None => {
                eprintln!("No SqlType value named {}", tyid.to_string());
                return quote!(compile_error!("No SqlType value with the given name");).into();
            }
        };

        if let Ok(item) = syn::parse::<ItemStruct>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: DeferredSqlType::Known(sqltype),
            });
        } else if let Ok(item) = syn::parse::<ItemEnum>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: DeferredSqlType::Known(sqltype),
            });
        }
    }

    match tyinfo {
        Some(tyinfo) => match add_custom_type(migrations_for_dir(), tyinfo.name, tyinfo.ty) {
            Ok(()) => input,
            Err(e) => {
                eprintln!("unable to save type {}", e);
                quote!(compile_error!("unable to save type");).into()
            }
        },
        None => {
            quote!(compile_error!("The #[propane_type] macro wasn't expected to be used here");)
                .into()
        }
    }
}

fn add_custom_type<M>(
    mut ms: impl MigrationsMut<M = M>,
    name: String,
    ty: DeferredSqlType,
) -> Result<()>
where
    M: MigrationMut,
{
    let current_migration = ms.current();
    let key = TypeKey::CustomType(name);
    current_migration.add_type(key, ty)
}

fn migrations_for_dir() -> migrations::FsMigrations {
    migrations::from_root(&migrations_dir())
}

fn migrations_dir() -> PathBuf {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push("propane");
    dir.push("migrations");
    dir
}

fn sqltype_from_name(name: &Ident) -> Option<SqlType> {
    let name = name.to_string();
    match name.as_ref() {
        "Bool" => Some(SqlType::Bool),
        "Int" => Some(SqlType::Int),
        "BigInt" => Some(SqlType::BigInt),
        "Real" => Some(SqlType::Real),
        "Text" => Some(SqlType::Text),
        #[cfg(feature = "datetime")]
        "Timestamp" => Some(SqlType::Timestamp),
        "Blob" => Some(SqlType::Blob),
        _ => None,
    }
}
