use crate::migrations::adb::{DeferredSqlType, TypeKey};
use crate::migrations::{MigrationMut, MigrationsMut};
use crate::{SqlType, SqlVal};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span, TokenTree};
use quote::{quote, ToTokens};
use syn::parse_quote;
use syn::{
    punctuated::Punctuated, Attribute, Field, ItemEnum, ItemStruct, ItemType, Lit, LitStr, Meta,
    MetaNameValue, NestedMeta,
};

#[macro_export]
macro_rules! make_compile_error {
    ($span:expr=> $($arg:tt)*) => ({
        let lit = crate::codegen::make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote_spanned!($span=> compile_error!(#lit))
    });
    ($($arg:tt)*) => ({
        let lit = crate::codegen::make_lit(&std::fmt::format(format_args!($($arg)*)));
        quote!(compile_error!(#lit))
    })
}

mod dbobj;
mod migration;

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

    let state_attrs = if has_derive_serialize(&attrs) {
        quote!(#[serde(skip)])
    } else {
        TokenStream2::new()
    };

    let vis = &ast_struct.vis;

    migration::write_table_to_disk(ms, &ast_struct, &config).unwrap();

    let impltraits = dbobj::impl_dbobject(&ast_struct, &config);
    let fieldexprs = dbobj::add_fieldexprs(&ast_struct);

    let fields: Punctuated<Field, syn::token::Comma> =
        match remove_helper_field_attributes(&mut ast_struct.fields) {
            Ok(fields) => fields.named.clone(),
            Err(err) => return err,
        };

    // If the program already declared a state field, remove it
    let fields = remove_existing_state_field(fields);

    let ident = ast_struct.ident;

    quote!(
        #(#attrs)*
        #vis struct #ident {
            #state_attrs
            pub state: butane::ObjectState,
            #fields
        }
        #impltraits
        #fieldexprs
    )
}

pub fn dataresult(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let dbo: Ident = syn::parse2(args)
        .expect("Model type must be specified as argument to dataresult attribute");
    let mut ast_struct: ItemStruct = syn::parse2(input).unwrap();

    // Filter out our helper attributes
    let attrs: Vec<Attribute> = filter_helper_attributes(&ast_struct);

    let state_attrs = if has_derive_serialize(&attrs) {
        quote!(#[serde(skip)])
    } else {
        TokenStream2::new()
    };

    let vis = &ast_struct.vis;

    let impltraits = dbobj::impl_dataresult(&ast_struct, &dbo);

    let fields = match remove_helper_field_attributes(&mut ast_struct.fields) {
        Ok(fields) => &fields.named,
        Err(err) => return err,
    };

    let ident = ast_struct.ident;

    quote!(
        #(#attrs)*
        #vis struct #ident {
            #state_attrs
            #fields
        }
        #impltraits
    )
}

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
        let args: TokenStream2 = args;
        let args: Vec<TokenTree> = args.into_iter().collect();
        if args.len() != 1 {
            return quote!(compile_error!("Expected butane_type(sqltype)"););
        }
        let tyid = match &args[0] {
            TokenTree::Ident(tyid) => tyid.clone(),
            _ => return quote!(compile_error!("Unexpected tokens in butane_type");),
        };
        let sqltype = match sqltype_from_name(&tyid) {
            Some(ty) => ty,
            None => {
                eprintln!("No SqlType value named {}", tyid.to_string());
                return quote!(compile_error!("No SqlType value with the given name"););
            }
        };

        if let Ok(item) = syn::parse2::<ItemStruct>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: DeferredSqlType::Known(sqltype),
            });
        } else if let Ok(item) = syn::parse2::<ItemEnum>(input.clone()) {
            tyinfo = Some(CustomTypeInfo {
                name: item.ident.to_string(),
                ty: DeferredSqlType::Known(sqltype),
            });
        }
    }

    match tyinfo {
        Some(tyinfo) => match add_custom_type(ms, tyinfo.name, tyinfo.ty) {
            Ok(()) => input,
            Err(e) => {
                eprintln!("unable to save type {}", e);
                quote!(compile_error!("unable to save type");)
            }
        },
        None => {
            quote!(compile_error!("The #[butane_type] macro wasn't expected to be used here");)
        }
    }
}

fn make_ident_literal_str(ident: &Ident) -> LitStr {
    let as_str = format!("{}", ident);
    LitStr::new(&as_str, Span::call_site())
}

pub fn make_lit(s: &str) -> LitStr {
    LitStr::new(s, Span::call_site())
}

fn filter_helper_attributes(ast_struct: &ItemStruct) -> Vec<Attribute> {
    ast_struct
        .attrs
        .clone()
        .into_iter()
        .filter(|a| !a.path.is_ident("table"))
        .collect()
}

fn config_from_attributes(ast_struct: &ItemStruct) -> dbobj::Config {
    let mut config = dbobj::Config::default();
    for attr in &ast_struct.attrs {
        if let Ok(Meta::NameValue(MetaNameValue {
            path,
            lit: Lit::Str(s),
            ..
        })) = attr.parse_meta()
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
                    !a.path.is_ident("pk")
                        && !a.path.is_ident("auto")
                        && !a.path.is_ident("sqltype")
                        && !a.path.is_ident("default")
                });
            }
            Ok(fields)
        }
        _ => Err(make_compile_error!("Fields must be named")),
    }
}

// We allow model structs to declare the state: butane::ObjectState
// field for convenience so it doesn't appear so magical, but then we
// recreate it.
fn remove_existing_state_field(
    fields: Punctuated<Field, syn::token::Comma>,
) -> Punctuated<Field, syn::token::Comma> {
    fields
        .into_iter()
        .filter(|f| match (&f.ident, &f.ty) {
            (Some(ident), syn::Type::Path(typ)) => {
                ident != "state"
                    || typ
                        .path
                        .segments
                        .last()
                        .map_or(true, |seg| seg.ident != "ObjectState")
            }
            (_, _) => true,
        })
        .collect()
}

fn pk_field(ast_struct: &ItemStruct) -> Option<Field> {
    let pk_by_attribute =
        fields(ast_struct).find(|f| f.attrs.iter().any(|attr| attr.path.is_ident("pk")));
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
    field.attrs.iter().any(|attr| attr.path.is_ident("auto"))
}

fn fields(ast_struct: &ItemStruct) -> impl Iterator<Item = &Field> {
    ast_struct
        .fields
        .iter()
        .filter(|f| f.ident.clone().unwrap() != "state")
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
    let seg = if path.segments.len() == 2 && path.segments.first().unwrap().ident == "butane" {
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
                .unwrap_or_else(|| panic!("{} must have an argument", tyname))
                .ident
                .to_string(),
        ))
    })
}

pub fn get_deferred_sql_type(ty: &syn::Type) -> DeferredSqlType {
    get_primitive_sql_type(ty)
        .or_else(|| get_option_sql_type(ty))
        .or_else(|| get_foreign_sql_type(ty, "ForeignKey"))
        .unwrap_or_else(|| {
            DeferredSqlType::Deferred(TypeKey::CustomType(
                ty.clone().into_token_stream().to_string(),
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
        .find(|attr| attr.path.is_ident("default"));
    let lit: Lit = match attr {
        None => return Ok(None),
        Some(attr) => match attr.parse_meta() {
            Ok(Meta::NameValue(meta)) => meta.lit,
            _ => return Err(make_compile_error!("malformed default value").into()),
        },
    };
    Ok(Some(sqlval_from_lit(lit)?))
}

/// If the field refers to a primitive, return its SqlType
fn get_primitive_sql_type(ty: &syn::Type) -> Option<DeferredSqlType> {
    if *ty == parse_quote!(bool) {
        return Some(DeferredSqlType::Known(SqlType::Bool));
    } else if *ty == parse_quote!(u8)
        || *ty == parse_quote!(i8)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i16)
        || *ty == parse_quote!(u16)
        || *ty == parse_quote!(i32)
    {
        return Some(DeferredSqlType::Known(SqlType::Int));
    } else if *ty == parse_quote!(u32) || *ty == parse_quote!(i64) {
        // Future improvement: better support unsigned integers
        // here. Sqlite has no u64, though Postgres does
        return Some(DeferredSqlType::Known(SqlType::BigInt));
    } else if *ty == parse_quote!(f32) || *ty == parse_quote!(f64) {
        return Some(DeferredSqlType::Known(SqlType::Real));
    } else if *ty == parse_quote!(String) {
        return Some(DeferredSqlType::Known(SqlType::Text));
    } else if *ty == parse_quote!(Vec<u8>) {
        return Some(DeferredSqlType::Known(SqlType::Blob));
    }

    #[cfg(feature = "datetime")]
    {
        if *ty == parse_quote!(NaiveDateTime) {
            return Some(DeferredSqlType::Known(SqlType::Timestamp));
        }
    }

    #[cfg(feature = "uuid")]
    {
        if *ty == parse_quote!(Uuid) || *ty == parse_quote!(uuid::Uuid) {
            return Some(DeferredSqlType::Known(SqlType::Blob));
        }
    }

    None
}

fn has_derive_serialize(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if let Ok(Meta::List(ml)) = attr.parse_meta() {
            if ml.path.is_ident("derive")
                && ml.nested.iter().any(|nm| match nm {
                    NestedMeta::Meta(Meta::Path(path)) => path.is_ident("Serialize"),
                    _ => false,
                })
            {
                return true;
            }
        }
    }
    false
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
    }
}

struct CustomTypeInfo {
    name: String,
    ty: DeferredSqlType,
}

fn add_custom_type<M>(
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

#[derive(Debug)]
struct CompilerErrorMsg {
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
