use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, Field, ItemStruct};

// implement the DBObject trait
pub fn impl_dbobject(ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let table_lit = make_ident_literal_str(&tyname);

    let columns = columns(ast_struct);

    let pk_field = pk_field(&ast_struct);
    if pk_field.is_none() {
        return quote_spanned! {
        ast_struct.span() =>
            compile_error!("No pk field found");
        };
    };
    let pk_field = pk_field.unwrap();
    let pktype = pk_field.ty;
    let pkident = pk_field.ident.unwrap();
    let pklit = make_ident_literal_str(&pkident);

    let rows = rows_for_from(&ast_struct);
    let rowslen = rows.len();

    quote!(
        impl propane::DBObject for #tyname {
            type PKType = #pktype;
            const COLUMNS: &'static [propane::db::Column] = &[
                #columns
            ];
            fn get(
                conn: &impl BackendConnection,
                id: Self::PKType,
            ) -> propane::Result<Self> {
                Self::query()
                    .filter(BoolExpr::Eq(#pklit, Expr::Val(#pkident.into())))
                    .limit(1)
                    .load(conn)?
                    .into_iter()
                    .nth(0)
                    .ok_or(propane::Error::NoSuchObject.into())
            }
            fn query() -> Query {
                Query::new(#table_lit)
            }
            fn from_row(mut row: propane::db::Row) -> propane::Result<Self> {
                if row.len() != #rowslen {
                    return Err(propane::Error::BoundsError.into());
                }
                let mut it = row.into_iter();
                Ok(#tyname {
                    #(#rows),*
                })
            }
        }
    )
}

pub fn add_fieldexprs_to_impl(ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let fieldexprs: Vec<TokenStream2> = ast_struct
        .fields
        .iter()
        .map(|f| {
            let fid = match &f.ident {
                Some(fid) => fid,
                None => {
                    return quote_spanned!(
                        f.span() =>
                            compile_error!("Fields must be named for propane");
                    )
                }
            };
            let fidlit = make_ident_literal_str(&fid);
            let fnid = Ident::new(&format!("fieldexpr_{}", fid), f.span());
            let fty = &f.ty;
            quote!(
                fn #fnid() -> propane::field::FieldExpr<#fty> {
                    propane::field::FieldExpr::<#fty>::new(#fidlit)
                }
            )
        })
        .collect();

    quote!(
        impl #tyname {
            #(#fieldexprs)*
        }
    )
}

fn rows_for_from(ast_struct: &ItemStruct) -> Vec<TokenStream2> {
    ast_struct.fields.iter().map(|f| from_row_cell(f)).collect()
}

fn from_row_cell(f: &Field) -> TokenStream2 {
    let ident = f.ident.clone().unwrap();
    quote!(#ident: it.next().unwrap().sql_into()?)
}

fn columns(ast_struct: &ItemStruct) -> TokenStream2 {
    ast_struct
        .fields
        .iter()
        .map(|f| match f.ident.clone() {
            Some(fname) => {
                let ident = make_ident_literal_str(&fname);
                let ty = tokens_for_sqltype(get_sql_type(&f));
                quote!(propane::db::Column::new(#ident, #ty),)
            }
            None => quote_spanned! {
                f.span() =>
                    compile_error!("Fields must be named for propane");
            },
        })
        .collect()
}

fn pk_field(ast_struct: &ItemStruct) -> Option<Field> {
    // todo support #[pk] attribute
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
