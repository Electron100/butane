use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Field, ItemStruct};

// implement the DBObject trait
pub fn impl_dbobject(ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let table_lit = make_ident_literal_str(&tyname);

    let columns = columns(ast_struct);

    let pk_field = pk_field(&ast_struct);
    if pk_field.is_none() {
        return make_compile_error!(ast_struct.span() => "No pk field found");
    };
    let pk_field = pk_field.unwrap();
    let pktype = pk_field.ty;
    let pkident = pk_field.ident.unwrap();
    let pklit = make_ident_literal_str(&pkident);
    let fields_type = fields_type(tyname);
    let tablelit = make_ident_literal_str(&tyname);

    let rows = rows_for_from(&ast_struct);
    let numfields = rows.len();

    let values: Vec<TokenStream2> = ast_struct
        .fields
        .iter()
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            quote!(values.push(propane::ToSql::to_sql(&self.#ident));)
        })
        .collect();

    quote!(
        impl propane::DBResult for #tyname {
            type DBO = #tyname;
            type Fields = #fields_type;
            const COLUMNS: &'static [propane::db::internal::Column] = &[
                #columns
            ];
            fn from_row(mut row: propane::db::internal::Row) -> propane::Result<Self> {
                if row.len() != #numfields {
                    return Err(propane::Error::BoundsError.into());
                }
                let mut it = row.into_iter();
                Ok(#tyname {
                    #(#rows),*
                })
            }
            fn query() -> propane::query::Query<Self> {
                propane::query::Query::new(#table_lit)
            }
        }
        impl propane::DBObject for #tyname {
            type PKType = #pktype;
            const PKCOL: &'static str = #pklit;
            const TABLE: &'static str = #tablelit;
            fn pk(&self) -> &Self::PKType {
                &self.#pkident
            }
            fn get(
                conn: &impl propane::db::internal::ConnectionMethods,
                id: Self::PKType,
            ) -> propane::Result<Self> {
                Self::query()
                    .filter(propane::query::BoolExpr::Eq(#pklit, propane::query::Expr::Val(id.into())))
                    .limit(1)
                    .load(conn)?
                    .into_iter()
                    .nth(0)
                    .ok_or(propane::Error::NoSuchObject.into())
            }
            fn save(&mut self, conn: &impl propane::db::internal::ConnectionMethods) -> propane::Result<()> {
                //todo perf use an array on the stack for better
                let mut values: Vec<propane::SqlVal> = Vec::with_capacity(#numfields);
                #(#values)*
                conn.insert_or_replace(Self::TABLE, <Self as propane::DBResult>::COLUMNS, &values)
            }
            fn delete(&self, conn: &impl propane::db::internal::ConnectionMethods) -> propane::Result<()> {
                use propane::ToSql;
                use propane::prelude::DBObject;
                conn.delete(Self::TABLE, Self::PKCOL, &self.pk().to_sql())
            }
        }
        impl propane::ToSql for #tyname {
            fn to_sql(&self) -> propane::SqlVal {
                use propane::DBObject;
                propane::ToSql::to_sql(self.pk())
            }
        }
        impl propane::ToSql for &#tyname {
            fn to_sql(&self) -> propane::SqlVal {
                use propane::DBObject;
                propane::ToSql::to_sql(self.pk())
            }
        }
        impl PartialEq<propane::ForeignKey<#tyname>> for #tyname {
            fn eq(&self, other: &propane::ForeignKey<#tyname>) -> bool {
                other.eq(&self)
            }
        }
        impl PartialEq<propane::ForeignKey<#tyname>> for &#tyname {
            fn eq(&self, other: &propane::ForeignKey<#tyname>) -> bool {
                other.eq(self)
            }
        }
    )
}

pub fn add_fieldexprs(ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let vis = &ast_struct.vis;
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
                #vis fn #fnid(&self) -> propane::query::FieldExpr<#fty> {
                    propane::query::FieldExpr::<#fty>::new(#fidlit)
                }
            )
        })
        .collect();

    let fields_type = fields_type(tyname);
    quote!(
        impl #tyname {
            pub fn fields() -> #fields_type {
                #fields_type::default()
            }
        }
        #vis struct #fields_type {
        }
        impl #fields_type {
            #(#fieldexprs)*
        }
        impl std::default::Default for #fields_type {
            fn default() -> Self {
                #fields_type{}
            }
        }
    )
}

fn fields_type(tyname: &Ident) -> Ident {
    Ident::new(&format!("{}Fields", tyname), Span::call_site())
}

fn rows_for_from(ast_struct: &ItemStruct) -> Vec<TokenStream2> {
    ast_struct.fields.iter().map(|f| from_row_cell(f)).collect()
}

fn from_row_cell(f: &Field) -> TokenStream2 {
    let ident = f.ident.clone().unwrap();
    quote!(#ident: propane::FromSql::from_sql(it.next().unwrap())?)
}

fn columns(ast_struct: &ItemStruct) -> TokenStream2 {
    ast_struct
        .fields
        .iter()
        .map(|f| match f.ident.clone() {
            Some(fname) => {
                let ident = make_ident_literal_str(&fname);
                let fty = &f.ty;
                quote!(propane::db::internal::Column::new(#ident, <#fty as propane::FieldType>::SQLTYPE),)
            }
            None => quote_spanned! {
                f.span() =>
                    compile_error!("Fields must be named for propane");
            },
        })
        .collect()
}
