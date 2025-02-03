//! Code-generation backend

use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use quote::{quote, ToTokens};
use regex::Regex;
use syn::parse_quote;
use syn::{
    punctuated::Punctuated, Attribute, Field, ItemEnum, ItemStruct, ItemType, Lit, LitStr, Meta,
    MetaNameValue,
};

use crate::migrations::adb::{DeferredSqlType, TypeIdentifier, TypeKey};
use crate::migrations::{MigrationMut, MigrationsMut};
use crate::{SqlType, SqlVal};

const OPTION_TYNAMES: [&str; 3] = ["Option", "option::Option", "std::option::Option"];
const MANY_TYNAMES: [&str; 2] = ["Many", "butane::Many"];
const FKEY_TYNAMES: [&str; 2] = ["ForeignKey", "butane::ForeignKey"];
const AUTOPK_TYNAMES: [&str; 2] = ["AutoPk", "butane::AutoPk"];

/// Create a compiler error.
#[macro_export]
macro_rules! make_compile_error {
    ($span:expr=> $($arg:tt)*) => ({
        let lit = $crate::codegen::make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote_spanned!($span=> compile_error!(#lit))
    });
    ($($arg:tt)*) => ({
        let lit = $crate::codegen::make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote!(compile_error!(#lit))
    })
}

mod dbobj;
mod migration;

/// Implementation of `#[butane::model]`.
pub fn model_with_migrations<M>(
    input: TokenStream2,
    ms: &mut impl MigrationsMut<M = M>,
) -> TokenStream2
where
    M: MigrationMut,
{
    // Transform into a derive because derives can have helper
    // attributes but proc macro attributes can't yet (nor can they
    // create field attributes)
    let mut ast_struct: ItemStruct = syn::parse2(input).unwrap();
    let config: dbobj::Config = config_from_attributes(&ast_struct);

    // Filter out our helper attributes
    let attrs: Vec<Attribute> = filter_helper_attributes(&ast_struct);

    let vis = &ast_struct.vis;

    migration::write_table_to_disk(ms, &ast_struct, &config).unwrap();

    let impltraits = dbobj::impl_dbobject(&ast_struct, &config);
    let fieldexprs = dbobj::add_fieldexprs(&ast_struct, &config);

    let fields: Punctuated<Field, syn::token::Comma> =
        match remove_helper_field_attributes(&mut ast_struct.fields) {
            Ok(fields) => fields.named.clone(),
            Err(err) => return err,
        };

    let ident = ast_struct.ident;

    quote!(
        #(#attrs)*
        #vis struct #ident {
            #fields
        }
        #impltraits
        #fieldexprs
    )
}

/// Implementation of `#[butane::dataresult(<Model>)]`.
pub fn dataresult(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let dbo: Ident = syn::parse2(args)
        .expect("Model type must be specified as argument to dataresult attribute");
    let mut ast_struct: ItemStruct = syn::parse2(input).unwrap();
    let config: dbobj::Config = config_from_attributes(&ast_struct);

    // Filter out our helper attributes
    let attrs: Vec<Attribute> = filter_helper_attributes(&ast_struct);

    let vis = &ast_struct.vis;

    let impltraits = dbobj::impl_dataresult(&ast_struct, &dbo, &config);

    let fields = match remove_helper_field_attributes(&mut ast_struct.fields) {
        Ok(fields) => &fields.named,
        Err(err) => return err,
    };

    let ident = ast_struct.ident;

    quote!(
        #(#attrs)*
        #vis struct #ident {
            #fields
        }
        #impltraits
    )
}

fn parse_butane_type_args(args: TokenStream2) -> std::result::Result<TypeIdentifier, TokenStream2> {
    let args: Vec<TokenTree> = args.into_iter().collect();
    if args.is_empty() {
        return Err(quote!(compile_error!("Expected butane_type(sqltype)");));
    }
    let tyid = match &args[0] {
        TokenTree::Ident(tyid) => tyid.clone(),
        _ => return Err(quote!(compile_error!("Unexpected tokens in butane_type");)),
    };
    if args.len() == 1 {
        return Ok(match sqltype_from_name(&tyid) {
            Some(ty) => ty,
            None => {
                eprintln!("No SqlType value named {tyid}");
                return Err(quote!(compile_error!("No SqlType value with the given name");));
            }
        });
    } else if tyid == "Custom" {
        let customerr = quote!(
            compile_error!("Unexpected tokens custom in butane_type. Expected butane_type(Custom(name)).");
        );
        return match args.get(1) {
            Some(TokenTree::Group(g)) if !g.stream().is_empty() => {
                let customid = g.stream().into_iter().nth(0).unwrap();
                match customid {
                    TokenTree::Ident(tyid) => Ok(TypeIdentifier::Name(tyid.to_string())),
                    _ => Err(customerr),
                }
            }
            _ => Err(customerr),
        };
    }
    Err(quote!(compile_error!("Unexpected tokens in butane_type");))
}

/// Implementation of `#[butane::butane_type(<SqlType>)]`.
pub fn butane_type_with_migrations<M>(
    args: TokenStream2,
    input: TokenStream2,
    ms: &mut impl MigrationsMut<M = M>,
) -> TokenStream2
where
    M: MigrationMut,
{
    let mut tyinfo: Option<CustomTypeInfo> = None;
    let type_alias: syn::Result<ItemType> = syn::parse2(input.clone());
    if let Ok(type_alias) = type_alias {
        tyinfo = Some(CustomTypeInfo {
            name: type_alias.ident.to_string(),
            ty: get_deferred_sql_type(&type_alias.ty),
        })
    }

    if tyinfo.is_none() {
        // For types below here, we need the SqlType given to us
        let sqltype = match parse_butane_type_args(args) {
            Ok(sqltype) => sqltype,
            Err(t) => return t,
        };
        if let Ok(item) = syn::parse2::<ItemStruct>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: sqltype.into(),
            });
        } else if let Ok(item) = syn::parse2::<ItemEnum>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: sqltype.into(),
            });
        }
    }

    match tyinfo {
        Some(tyinfo) => match add_custom_type(ms, tyinfo.name, tyinfo.ty) {
            Ok(()) => input,
            Err(e) => {
                eprintln!("unable to save type {e}");
                quote!(compile_error!("unable to save type");)
            }
        },
        None => {
            quote!(compile_error!("The #[butane_type] macro wasn't expected to be used here");)
        }
    }
}

/// Create a [`struct@LitStr`] (UTF-8 string literal) from an [Ident].
pub fn make_ident_literal_str(ident: &Ident) -> LitStr {
    let as_str = format!("{ident}");
    make_lit(&as_str)
}

/// Create a [`struct@LitStr`] (UTF-8 string literal) from a `str`.
pub fn make_lit(s: &str) -> LitStr {
    LitStr::new(s, Span::call_site())
}

fn filter_helper_attributes(ast_struct: &ItemStruct) -> Vec<Attribute> {
    ast_struct
        .attrs
        .clone()
        .into_iter()
        .filter(|a| !a.path().is_ident("table"))
        .collect()
}

fn config_from_attributes(ast_struct: &ItemStruct) -> dbobj::Config {
    let mut config = dbobj::Config::default();
    for attr in &ast_struct.attrs {
        // #[table = "name"]
        if let Meta::NameValue(MetaNameValue {
            path,
            value: syn::Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }),
            ..
        }) = &attr.meta
        {
            if path.is_ident("table") {
                config.table_name = Some(s.value())
            }
        }
    }
    config
}

fn remove_helper_field_attributes(
    fields: &mut syn::Fields,
) -> std::result::Result<&syn::FieldsNamed, TokenStream2> {
    match fields {
        syn::Fields::Named(fields) => {
            for field in &mut fields.named {
                field.attrs.retain(|a| {
                    !a.path().is_ident("pk")
                        && !a.path().is_ident("sqltype")
                        && !a.path().is_ident("default")
                        && !a.path().is_ident("unique")
                });
            }
            Ok(fields)
        }
        _ => Err(make_compile_error!("Fields must be named")),
    }
}

fn pk_field(ast_struct: &ItemStruct) -> Option<Field> {
    let pk_by_attribute =
        fields(ast_struct).find(|f| f.attrs.iter().any(|attr| attr.path().is_ident("pk")));
    if let Some(id_field) = pk_by_attribute {
        return Some(id_field.clone());
    }
    let pk_by_name = ast_struct.fields.iter().find(|f| match &f.ident {
        Some(ident) => *ident == "id",
        None => false,
    });
    pk_by_name.cloned()
}

fn is_auto(field: &Field) -> bool {
    get_type_argument(&field.ty, &AUTOPK_TYNAMES).is_some()
}

fn is_unique(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("unique"))
}

fn fields(ast_struct: &ItemStruct) -> impl Iterator<Item = &Field> {
    ast_struct.fields.iter()
}

fn get_option_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    get_type_argument(ty, &OPTION_TYNAMES).map(|path| {
        let inner_ty: syn::Type = syn::TypePath {
            qself: None,
            path: path.clone(),
        }
        .into();

        get_deferred_sql_type(&inner_ty)
    })
}

fn get_foreign_key_sql_type(field: &Field) -> Option<DeferredSqlType> {
    if let Some(inner_type_path) = get_type_argument(&field.ty, &OPTION_TYNAMES) {
        let inner_ty: syn::Type = syn::TypePath {
            qself: None,
            path: inner_type_path.clone(),
        }
        .into();

        return get_foreign_sql_type(&inner_ty, &FKEY_TYNAMES);
    }
    get_foreign_sql_type(&field.ty, &FKEY_TYNAMES)
}

fn get_many_sql_type(field: &Field) -> Option<DeferredSqlType> {
    get_foreign_sql_type(&field.ty, &MANY_TYNAMES)
}

fn get_autopk_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    get_type_argument(ty, &AUTOPK_TYNAMES).map(|path| {
        let inner_ty: syn::Type = syn::TypePath {
            qself: None,
            path: path.clone(),
        }
        .into();

        get_deferred_sql_type(&inner_ty)
    })
}

fn is_many_to_many(field: &Field) -> bool {
    get_many_sql_type(field).is_some()
}

fn is_foreign_key(field: &Field) -> bool {
    get_foreign_key_sql_type(field).is_some()
}

fn is_option(field: &Field) -> bool {
    get_type_argument(&field.ty, &OPTION_TYNAMES).is_some()
}

/// Check for special fields which won't correspond to rows and don't
/// implement FieldType
fn is_row_field(f: &Field) -> bool {
    !is_many_to_many(f)
}

/// Test if the ident of each segment in two paths is the same without
/// looking at the arguments.
fn is_same_path_ident(path1: &syn::Path, path2: &syn::Path) -> bool {
    if path1.segments.len() != path2.segments.len() {
        return false;
    }
    path1
        .segments
        .iter()
        .zip(path2.segments.iter())
        .all(|(a, b)| a.ident == b.ident)
}

/// Gets the type argument of a type.
/// E.g. for Foo<T>, returns T
fn get_type_argument<'a>(ty: &'a syn::Type, tynames: &[&'static str]) -> Option<&'a syn::Path> {
    let path = match ty {
        syn::Type::Path(path) => &path.path,
        _ => return None,
    };
    if !tynames
        .iter()
        .any(|tyname| match syn::parse_str::<syn::Path>(tyname) {
            Ok(ty_path) => is_same_path_ident(path, &ty_path),
            // Should only happen if there's a bug in butane
            Err(_) => panic!("Cannot parse {tyname} as syn::Path"),
        })
    {
        return None;
    }
    let seg = path.segments.last().unwrap();
    let args = match &seg.arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return None,
    };
    if args.len() != 1 {
        panic!("{} should have a single type argument", tynames[0])
    }
    match args.last().unwrap() {
        syn::GenericArgument::Type(syn::Type::Path(typath)) => Some(&typath.path),
        _ => panic!("{} argument should be a type.", tynames[0]),
    }
}

fn get_foreign_sql_type(ty: &syn::Type, tynames: &[&'static str]) -> Option<DeferredSqlType> {
    let typath = get_type_argument(ty, tynames);
    typath.map(|typath| {
        DeferredSqlType::Deferred(TypeKey::PK(
            typath
                .segments
                .last()
                .unwrap_or_else(|| panic!("{} must have an argument", tynames[0]))
                .ident
                .to_string(),
        ))
    })
}

/// Determine whether a type refers to a data type that is supported directly by butane,
/// or is a custom defined struct.
/// It looks inside an [Option] or [crate::fkey::ForeignKey] to determine the inner type.
pub fn get_deferred_sql_type(ty: &syn::Type) -> DeferredSqlType {
    get_primitive_sql_type(ty)
        .or_else(|| get_option_sql_type(ty))
        .or_else(|| get_foreign_sql_type(ty, &FKEY_TYNAMES))
        .or_else(|| get_autopk_sql_type(ty))
        .unwrap_or_else(|| {
            DeferredSqlType::Deferred(TypeKey::CustomType(
                ty.clone().into_token_stream().to_string().replace(' ', ""),
            ))
        })
}

/// Defaults are used for fields added by later migrations
/// Example
/// #[default = 42]
fn get_default(field: &Field) -> std::result::Result<Option<SqlVal>, CompilerErrorMsg> {
    let attr: Option<&Attribute> = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("default"));
    let lit: Lit = match attr {
        None => return Ok(None),
        Some(attr) => match &attr.meta {
            Meta::NameValue(MetaNameValue {
                value: syn::Expr::Lit(expr_lit),
                ..
            }) => expr_lit.lit.clone(),
            _ => return Err(make_compile_error!("malformed default value").into()),
        },
    };
    Ok(Some(sqlval_from_lit(lit)?))
}

fn some_id(ty: SqlType) -> Option<TypeIdentifier> {
    Some(TypeIdentifier::Ty(ty))
}

fn some_known(ty: SqlType) -> Option<DeferredSqlType> {
    Some(DeferredSqlType::KnownId(TypeIdentifier::Ty(ty)))
}

/// If the field refers to a primitive, return its SqlType
pub fn get_primitive_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    if *ty == parse_quote!(bool) {
        return some_known(SqlType::Bool);
    } else if *ty == parse_quote!(u8)
        || *ty == parse_quote!(i8)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i16)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i32)
    {
        return some_known(SqlType::Int);
    } else if *ty == parse_quote!(u32) || *ty == parse_quote!(i64) {
        // Future improvement: better support unsigned integers
        // here. Sqlite has no u64, though Postgres does
        return some_known(SqlType::BigInt);
    } else if *ty == parse_quote!(f32) || *ty == parse_quote!(f64) {
        return some_known(SqlType::Real);
    } else if *ty == parse_quote!(String)
        || *ty == parse_quote!(std::string::String)
        || *ty == parse_quote!(::std::string::String)
    {
        return some_known(SqlType::Text);
    } else if *ty == parse_quote!(Vec<u8>)
        || *ty == parse_quote!(std::vec::Vec<u8>)
        || *ty == parse_quote!(::std::vec::Vec<u8>)
    {
        return some_known(SqlType::Blob);
    }

    #[cfg(feature = "json")]
    {
        if *ty == parse_quote!(serde_json::Value) || *ty == parse_quote!(Value) {
            return some_known(SqlType::Json);
        }
    }

    #[cfg(feature = "datetime")]
    {
        // Note, the fact that we have to check specific paths because
        // we don't really have type system information at this point
        // is a strong argument for proc macros being the wrong time
        // to run the full migration generation. We expect these types
        // to come from chrono, but we don't really know for sure...
        if let Some(syn::PathSegment { ident, arguments }) = last_path_segment(ty) {
            match ident.to_string().as_str() {
                "NaiveDateTime" => return some_known(SqlType::Timestamp),
                "DateTime" => {
                    // Only if the parameter is UTC, as we don't support attached
                    // time zones
                    if template_type(arguments)
                        .map(|ident| ident.to_string())
                        .unwrap_or_default()
                        == "Utc"
                    {
                        return some_known(SqlType::Timestamp);
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(feature = "uuid")]
    {
        if *ty == parse_quote!(Uuid) || *ty == parse_quote!(uuid::Uuid) {
            return some_known(SqlType::Blob);
        }
    }

    None
}

#[cfg(feature = "datetime")]
fn last_path_segment(ty: &syn::Type) -> Option<&syn::PathSegment> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        return segments.last();
    }
    None
}

#[cfg(feature = "datetime")]
fn template_type(arguments: &syn::PathArguments) -> Option<&Ident> {
    if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
        args, ..
    }) = arguments
    {
        if let Some(syn::GenericArgument::Type(template_ty)) = args.last() {
            if let Some(syn::PathSegment { ident, .. }) = last_path_segment(template_ty) {
                return Some(ident);
            }
        }
    }
    None
}

fn sqlval_from_lit(lit: Lit) -> std::result::Result<SqlVal, CompilerErrorMsg> {
    match lit {
        Lit::Str(lit) => Ok(SqlVal::Text(lit.value())),
        Lit::ByteStr(lit) => Ok(SqlVal::Blob(lit.value())),
        Lit::Byte(_) => Err(make_compile_error!("single byte literal is not supported").into()),
        Lit::Char(_) => Err(make_compile_error!("single char literal is not supported").into()),
        Lit::Int(lit) => Ok(SqlVal::Int(lit.base10_parse().unwrap())),
        Lit::Float(lit) => Ok(SqlVal::Real(lit.base10_parse().unwrap())),
        Lit::Bool(lit) => Ok(SqlVal::Bool(lit.value)),
        Lit::Verbatim(_) => {
            Err(make_compile_error!("raw verbatim literals are not supported").into())
        }
        _ => Err(make_compile_error!("unsupported literal").into()),
    }
}

#[derive(Debug)]
struct CustomTypeInfo {
    name: String,
    ty: DeferredSqlType,
}

/// Records the SqlType of a custom named type to the current migration.
pub fn add_custom_type<M>(
    ms: &mut impl MigrationsMut<M = M>,
    name: String,
    ty: DeferredSqlType,
) -> crate::Result<()>
where
    M: MigrationMut,
{
    let current_migration = ms.current();
    let key = TypeKey::CustomType(name);
    current_migration.add_type(key, ty)
}

fn sqltype_from_name(name: &Ident) -> Option<TypeIdentifier> {
    let name = name.to_string();
    match name.as_ref() {
        "Bool" => return some_id(SqlType::Bool),
        "Int" => return some_id(SqlType::Int),
        "BigInt" => return some_id(SqlType::BigInt),
        "Real" => return some_id(SqlType::Real),
        "Text" => return some_id(SqlType::Text),
        "Blob" => return some_id(SqlType::Blob),
        #[cfg(feature = "json")]
        "Json" => return some_id(SqlType::Json),
        #[cfg(feature = "datetime")]
        "Timestamp" => return some_id(SqlType::Timestamp),
        _ => (),
    }
    if let Some(custom_name) = Regex::new(r"^Custom\((.*)\)$").unwrap().captures(&name) {
        if let Some(pg_name) = Regex::new(r"^Pg\((.*)\)$")
            .unwrap()
            .captures(custom_name.get(1).unwrap().as_str())
        {
            return Some(TypeIdentifier::Name(
                pg_name.get(1).unwrap().as_str().to_string(),
            ));
        }
    }
    None
}

#[derive(Debug)]
struct CompilerErrorMsg {
    #[allow(unused)] // better compiler error reporting is an area of future improvement
    ts: TokenStream2,
}
impl CompilerErrorMsg {
    fn new(ts: TokenStream2) -> Self {
        CompilerErrorMsg { ts }
    }
}
impl From<TokenStream2> for CompilerErrorMsg {
    fn from(ts: TokenStream2) -> Self {
        CompilerErrorMsg::new(ts)
    }
}

#[cfg(test)]
mod tests {
    use syn::parse::Parser;

    use super::*;

    #[test]
    fn test_get_type_argument_option() {
        let expected_type_path: syn::Path = syn::parse_quote!(butane::ForeignKey<Foo>);

        let type_path: syn::TypePath = syn::parse_quote!(Option<butane::ForeignKey<Foo>>);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &OPTION_TYNAMES);
        assert!(rv.is_some());
        assert_eq!(rv.unwrap(), &expected_type_path);

        let type_path: syn::TypePath = syn::parse_quote!(butane::ForeignKey<Foo>);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &OPTION_TYNAMES);

        assert!(rv.is_none());
    }

    #[test]
    fn test_get_type_argument_fky() {
        let expected_type_path: syn::Path = syn::parse_quote!(Foo);

        let type_path: syn::TypePath = syn::parse_quote!(butane::ForeignKey<Foo>);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &FKEY_TYNAMES);
        assert!(rv.is_some());
        assert_eq!(rv.unwrap(), &expected_type_path);

        let type_path: syn::TypePath = syn::parse_quote!(Foo);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &FKEY_TYNAMES);
        assert!(rv.is_none());
    }

    #[test]
    fn test_get_type_argument_many() {
        let expected_type_path: syn::Path = syn::parse_quote!(Foo);

        let type_path: syn::TypePath = syn::parse_quote!(butane::Many<Foo>);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &MANY_TYNAMES);
        assert!(rv.is_some());
        assert_eq!(rv.unwrap(), &expected_type_path);

        let type_path: syn::TypePath = syn::parse_quote!(Foo);
        let typ = syn::Type::Path(type_path);
        let rv = get_type_argument(&typ, &MANY_TYNAMES);
        assert!(rv.is_none());
    }

    #[test]
    fn test_is_foreign_key() {
        let tokens = quote::quote! {
            foos: butane::ForeignKey<Foo>
        };
        let field = syn::Field::parse_named.parse2(tokens).unwrap();
        assert!(is_foreign_key(&field));

        let tokens = quote::quote! {
            foos: Option<butane::ForeignKey<Foo>>
        };
        let field = syn::Field::parse_named.parse2(tokens).unwrap();
        assert!(is_foreign_key(&field));

        let tokens = quote::quote! {
            foos: i8
        };
        let field = syn::Field::parse_named.parse2(tokens).unwrap();
        assert!(!is_foreign_key(&field));

        let tokens = quote::quote! {
            foos: Option<i8>
        };
        let field = syn::Field::parse_named.parse2(tokens).unwrap();
        assert!(!is_foreign_key(&field));

        let tokens = quote::quote! {
            foos: Option<SomethingElse>
        };
        let field = syn::Field::parse_named.parse2(tokens).unwrap();
        assert!(!is_foreign_key(&field));
    }
}
