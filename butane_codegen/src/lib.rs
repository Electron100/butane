//The quote macro can require a high recursion limit
#![recursion_limit = "256"]

extern crate proc_macro;

use butane_core::migrations::adb::{DeferredSqlType, TypeIdentifier};
use butane_core::{codegen, make_compile_error, migrations, SqlType};
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
///    initialized based on serial/auto-increment. Currently supported
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

#[proc_macro_derive(FieldType)]
pub fn derive_field_type(input: TokenStream) -> TokenStream {
    let derive_input = syn::parse_macro_input!(input as syn::DeriveInput);
    let ident = &derive_input.ident;
    match derive_input.data {
        syn::Data::Struct(_) => derive_field_type_with_json(ident),
        syn::Data::Enum(data_enum) => derive_field_type_for_enum(ident, data_enum),
        syn::Data::Union(_) => derive_field_type_with_json(ident),
    }
}

fn derive_field_type_for_enum(ident: &Ident, data_enum: syn::DataEnum) -> TokenStream {
    if data_enum
        .variants
        .iter()
        .any(|variant| variant.fields != syn::Fields::Unit)
    {
        // Non-simple enum, fall back to json derive
        return derive_field_type_with_json(ident);
    }

    let mut migrations = migrations_for_dir();

    codegen::add_custom_type(
        &mut migrations,
        ident.to_string(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    )
    .unwrap();

    let match_arms_to_string: Vec<TokenStream2> = data_enum
        .variants
        .iter()
        .map(|variant| {
            let v_ident = &variant.ident;
            let ident_literal = codegen::make_lit(&v_ident.to_string());
            quote!(Self::#v_ident => #ident_literal,)
        })
        .collect();
    let match_arms_from_string: Vec<TokenStream2> = data_enum
        .variants
        .iter()
        .map(|variant| {
            let v_ident = &variant.ident;
            let ident_literal = codegen::make_lit(&v_ident.to_string());
            quote!(#ident_literal => Ok(Self::#v_ident),)
        })
        .collect();
    quote!(
        impl #ident {
            fn to_string_for_butane(&self) -> &'static str {
                match self {
                    #(#match_arms_to_string)*
                }
            }
            fn from_string_for_butane(s: &str) -> std::result::Result<Self, butane::Error> {
                match s {
                    #(#match_arms_from_string)*
                    _ => Err(butane::Error::UnknownEnumVariant(s.to_string()))
                }
            }
        }
        impl butane::ToSql for #ident
        {
            fn to_sql(&self) -> butane::SqlVal {
                butane::SqlVal::Text(self.to_string_for_butane().to_string())
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
                butane::SqlValRef::Text(self.to_string_for_butane())
            }
        }

        impl butane::FromSql for #ident
        {
            fn from_sql_ref(val: butane::SqlValRef) -> std::result::Result<Self, butane::Error> {
                if let butane::SqlValRef::Text(v) = val {
                    return Self::from_string_for_butane(v);
                }
                Err(butane::Error::CannotConvertSqlVal(
                    butane::SqlType::Text,
                    val.into(),
                ))
            }
        }
        impl butane::FieldType for #ident
        {
            type RefType = Self;
            const SQLTYPE: butane::SqlType = butane::SqlType::Text;
        }
    )
    .into()
}

#[cfg(feature = "json")]
fn derive_field_type_with_json(struct_name: &Ident) -> TokenStream {
    let mut migrations = migrations_for_dir();

    codegen::add_custom_type(
        &mut migrations,
        struct_name.to_string(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Json)),
    )
    .unwrap();
    quote!(
        impl butane::ToSql for #struct_name
        {
            fn to_sql(&self) -> butane::SqlVal {
                self.to_sql_ref().into()
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
                butane::SqlValRef::Json(serde_json::to_value(self).unwrap())
            }
        }

        impl butane::FromSql for #struct_name
        {
            fn from_sql_ref(val: butane::SqlValRef) -> std::result::Result<Self, butane::Error> {
                if let butane::SqlValRef::Json(v) = val {
                    return Ok(#struct_name::deserialize(v).unwrap());
                }
                Err(butane::Error::CannotConvertSqlVal(
                    butane::SqlType::Json,
                    val.into(),
                ))
            }
        }
        impl butane::FieldType for #struct_name
        {
            type RefType = Self;
            const SQLTYPE: butane::SqlType = butane::SqlType::Json;
        }
    )
    .into()
}

#[cfg(not(feature = "json"))]
fn derive_field_type_with_json(_struct_name: &Ident) -> TokenStream {
    panic!("Feature 'json' is required to derive FieldType")
}
