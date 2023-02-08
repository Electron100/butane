//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use butane_core::*;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::TokenTree;
use quote::quote;
use std::path::PathBuf;
use syn::{Expr, Ident};

mod filter;

/// Attribute macro which marks a struct as being a data model and
/// generates an implementation of [`DataObject`](butane_core::DataObject). This
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
/// * `#[unique]` on a field indicates that the field's value must be unique
///    (perhaps implemented as the SQL UNIQUE constraint by some backends).
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
/// [`Many`]: butane_core::many::Many
#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    codegen::model_with_migrations(input.into(), &mut migrations_for_dir()).into()
}

/// Attribute macro which generates an implementation of
/// [`DataResult`](butane_core::DataResult). Continuing with our blog
/// post example from [model](macro@model), we could create a `DataResult` with
/// only some of the fields from `Post` (to avoid fetching all of them in a query).
///
/// ```ignore
/// #[dataresult(Post)]
/// pub struct PostMetadata {
///   pub id: i64,
///   pub title: String,
///   pub pub_time: Option<NaiveDateTime>,
/// }
/// ```
///
/// Note that the attribute takes a parameter saying which Model this
/// result is a subset of. Every field named in the DataResult must be
/// present in the Model.
#[proc_macro_attribute]
pub fn dataresult(args: TokenStream, input: TokenStream) -> TokenStream {
    codegen::dataresult(args.into(), input.into()).into()
}

#[proc_macro]
pub fn filter(input: TokenStream) -> TokenStream {
    let input: TokenStream2 = input.into();
    let args: Vec<TokenTree> = input.into_iter().collect();
    if args.len() < 2 {
        return make_compile_error!("Expected filter!(Type, expression)").into();
    }
    let tyid: Ident = match &args[0] {
        TokenTree::Ident(tyid) => tyid.clone(),
        TokenTree::Group(g) => match syn::parse2::<Ident>(g.stream()) {
            Ok(ident) => ident,
            Err(_) => {
                return make_compile_error!("Unexpected tokens in database object type {:?}", &g)
                    .into()
            }
        },
        _ => {
            return make_compile_error!("Unexpected tokens in database object type {:?}", &args[0])
                .into()
        }
    };

    if let TokenTree::Punct(_) = args[1] {
    } else {
        return make_compile_error!("Expected filter!(Type, expression)").into();
    }

    let expr: TokenStream2 = args.into_iter().skip(2).collect();
    let expr: Expr = match syn::parse2(expr) {
        Ok(expr) => expr,
        Err(_) => {
            return make_compile_error!(
                "Expected filter!(Type, expression) but could not parse expression"
            )
            .into()
        }
    };
    filter::for_expr(&tyid, &expr).into()
}

/// Attribute macro which marks a type as being available to butane
/// for use in models.
///
/// May be used on type aliases, structs, or enums. Except when used
/// on type aliases, it must be given a parameter specifying the
/// SqlType it can be converted to.
///
/// E.g.
/// ```ignore
/// #[butane_type]
/// pub type CurrencyAmount = f64;
///
/// #[butane_type(Text)]
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
pub fn butane_type(args: TokenStream, input: TokenStream) -> TokenStream {
    codegen::butane_type_with_migrations(args.into(), input.into(), &mut migrations_for_dir())
        .into()
}

fn migrations_for_dir() -> migrations::FsMigrations {
    migrations::from_root(migrations_dir())
}

fn migrations_dir() -> PathBuf {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push(".butane");
    dir.push("migrations");
    dir
}

#[proc_macro_derive(ButaneJson)]
pub fn derive_butane_json(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let struct_name = &ast.ident;
    let s = format!(
        "impl ToSql for {struct_name}
{{
    fn to_sql(&self) -> SqlVal {{
        self.to_sql_ref().into()
    }}
    fn to_sql_ref(&self) -> SqlValRef<'_> {{
        SqlValRef::Json(serde_json::to_value(self).unwrap())
    }}
}}

impl FromSql for {struct_name}
{{
    fn from_sql_ref(val: SqlValRef) -> Result<Self, butane::Error> {{
        if let SqlValRef::Json(v) = val {{
            return Ok({struct_name}::deserialize(v).unwrap());
        }}
        Err(butane::Error::CannotConvertSqlVal(
            SqlType::Json,
            val.into(),
        ))
    }}
}}
impl FieldType for {struct_name}
{{
    type RefType = Self;
    const SQLTYPE: SqlType = SqlType::Json;
}}"
    );
    s.parse().unwrap()
}
