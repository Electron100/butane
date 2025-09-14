//! dbobj implementation dealing with DataObject::Fields

use super::{fields, get_type_argument, is_many_to_many, make_ident_literal_str, many_table_lit};
use crate::codegen::{
    dbobj::Config, get_default_lit, is_auto, is_row_field, is_unique, pk_field, MANY_TYNAMES,
};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, Field, ItemStruct};

/// Create the tokens to define the DataObject::Fields type for a given model.
pub fn fields_type_tokens(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let vis = &ast_struct.vis;
    let fieldexprs: Vec<TokenStream2> = fields(ast_struct)
        .map(|f| {
            if is_many_to_many(f) {
                fieldexpr_func_many(f, ast_struct, config)
            } else {
                fieldexpr_func_regular(f, ast_struct)
            }
        })
        .collect();

    let fields_type = fields_type(tyname);
    let num_row_fields = fields(ast_struct).filter(|f| is_row_field(f)).count();
    let field_defs: Vec<TokenStream2> = fields(ast_struct)
        .filter(|f| is_row_field(f))
        .map(|f| data_object_field_def_tokens(f, ast_struct))
        .collect();
    quote!(
        /// Helper struct for butane model.
        #vis struct #fields_type {
            defs: [butane::implementation::DataObjectFieldDef<#tyname>; #num_row_fields],
        }
        impl #fields_type {
            #(#fieldexprs)*
        }
        impl std::default::Default for #fields_type {
            fn default() -> Self {
                #fields_type{
                    defs: [
                        #(#field_defs),*
                    ]
                }
            }
        }
        impl butane::implementation::DataObjectFields for #fields_type {
            type DBO = #tyname;
            type IntoFieldsIter<'a> = &'a [butane::implementation::DataObjectFieldDef<#tyname>; #num_row_fields];
            fn field_defs(&self) -> Self::IntoFieldsIter<'_> {
                &self.defs
            }
        }
    )
}

pub(super) fn fields_type(tyname: &Ident) -> Ident {
    Ident::new(&format!("{tyname}Fields"), Span::call_site())
}

macro_rules! field_name {
    ($f:tt) => ({
        match &$f.ident {
            Some(fid) => fid,
            None => {
                return quote_spanned!(
                    $f.span() =>
                        compile_error!("Fields must be named for butane");
                );
            }
        }
    })
}

fn fieldexpr_func_regular(f: &Field, ast_struct: &ItemStruct) -> TokenStream2 {
    let fty = &f.ty;
    let fidlit = field_ident_lit(f);
    fieldexpr_func(
        f,
        ast_struct,
        quote!(butane::query::FieldExpr<#fty>),
        quote!(butane::query::FieldExpr::<#fty>::new(#fidlit)),
    )
}

fn fieldexpr_func_many(f: &Field, ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let fty = get_type_argument(&f.ty, &MANY_TYNAMES).expect("Many field misdetected");
    let many_table_lit = many_table_lit(ast_struct, f, config);
    fieldexpr_func(
        f,
        ast_struct,
        quote!(butane::query::ManyFieldExpr<#tyname, #fty>),
        quote!(butane::query::ManyFieldExpr::<#tyname, #fty>::new(#many_table_lit)),
    )
}

fn fieldexpr_func(
    f: &Field,
    ast_struct: &ItemStruct,
    field_expr_type: TokenStream2,
    field_expr_ctor: TokenStream2,
) -> TokenStream2 {
    let vis = &ast_struct.vis;
    let fid = field_name!(f);
    let fnid = Ident::new(&format!("{fid}"), f.span());
    quote!(
        /// Create query expression.
        #vis fn #fnid(&self) -> #field_expr_type {
            #field_expr_ctor
        }
    )
}

fn field_ident_lit(f: &Field) -> TokenStream2 {
    let fid = field_name!(f);
    make_ident_literal_str(fid).into_token_stream()
}

/// Emits the tokens to construct a DataObjectFieldDef for a given [Field] from an [ItemStruct]
fn data_object_field_def_tokens(f: &Field, ast_struct: &ItemStruct) -> TokenStream2 {
    let dbo = ast_struct.ident.to_token_stream();
    let name = field_ident_lit(f);
    let field_type = f.ty.to_token_stream();
    let sqltype = quote!(<#field_type as butane::FieldType>::SQLTYPE);
    let nullable = quote!(<#field_type as butane::FieldType>::NULLABLE);
    let pk = pk_field(ast_struct)
        .expect("No primary key found. Expected 'id' field or field with #[pk] attribute.");
    let is_pk = (f == &pk).to_token_stream();
    let auto = is_auto(f).to_token_stream();
    let unique = is_unique(f).to_token_stream();
    let default = get_default_lit(f)
        .expect("Malformed default attribute")
        .map(|lit| quote!(Some(butane::ToSql::to_sql(#lit))))
        .unwrap_or_else(|| quote!(None));
    quote!(
        butane::implementation::DataObjectFieldDef::<#dbo>::builder()
                        .name(#name)
                        .sqltype(#sqltype)
                        .nullable(#nullable)
                        .pk(#is_pk)
                        .auto(#auto)
                        .unique(#unique)
                        .default(#default)
                        .build()
    )
}
