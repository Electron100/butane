use super::*;
use crate::migrations::adb::DeferredSqlType;
use crate::SqlType;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Field, ItemStruct};

// Configuration that can be specified with attributes to override default behavior
#[derive(Default)]
pub struct Config {
    pub table_name: Option<String>,
}

// implement the DataObject trait
pub fn impl_dbobject(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let tablelit = make_tablelit(config, tyname);

    let err = verify_fields(ast_struct);
    if let Some(err) = err {
        return err;
    }

    let pk_field = pk_field(&ast_struct).unwrap();
    let pktype = &pk_field.ty;
    let pkident = pk_field.ident.clone().unwrap();
    let pklit = make_ident_literal_str(&pkident);

    let insert_cols = columns(ast_struct, |f| !is_auto(f));
    let save_cols = columns(ast_struct, |f| !is_auto(f) && f != &pk_field);

    let mut post_insert: Vec<TokenStream2> = Vec::new();
    add_post_insert_for_auto(&pk_field, &mut post_insert);
    post_insert.push(quote!(self.state.saved = true;));

    let numdbfields = fields(&ast_struct).filter(|f| is_row_field(f)).count();
    let many_save: TokenStream2 = fields(&ast_struct).filter(|f| is_many_to_many(f)).map(|f| {
        let ident = f.ident.clone().expect("Fields must be named for propane");
        let many_table_lit = many_table_lit(&ast_struct, f);
        let pksqltype =
            quote!(<<Self as propane::DataObject>::PKType as propane::FieldType>::SQLTYPE);
        // Save  needs to ensure_initialized
        quote!(
            self.#ident.ensure_init(#many_table_lit, propane::ToSql::to_sql(self.pk()), #pksqltype);
            self.#ident.save(conn)?;
        )
    }).collect();

    let values: Vec<TokenStream2> = push_values(&ast_struct, |_| true);
    let values_no_pk: Vec<TokenStream2> = push_values(&ast_struct, |f: &Field| f != &pk_field);

    let dataresult = impl_dataresult(ast_struct, config);
    quote!(
                #dataresult
        impl propane::DataObject for #tyname {
            type PKType = #pktype;
            const PKCOL: &'static str = #pklit;
            const TABLE: &'static str = #tablelit;
            fn pk(&self) -> &Self::PKType {
                &self.#pkident
            }
            fn save(&mut self, conn: &impl propane::db::ConnectionMethods) -> propane::Result<()> {
                #many_save
                //todo perf use an array on the stack for better
                let mut values: Vec<propane::SqlVal> = Vec::with_capacity(#numdbfields);
                let pkcol = propane::db::Column::new(
                    #pklit,
                    <#pktype as propane::FieldType>::SQLTYPE);
                if self.state.saved {
                    #(#values_no_pk)*
                    if values.len() > 0 {
                        conn.update(Self::TABLE,
                                    pkcol,
                                    propane::ToSql::to_sql(self.pk()),
                                    &[#save_cols], &values)?;
                    }
                } else {
                    #(#values)*
                    let pk = conn.insert(Self::TABLE, &[#insert_cols], pkcol, &values)?;
                    #(#post_insert)*
                }
                Ok(())
            }
            fn delete(&self, conn: &impl propane::db::ConnectionMethods) -> propane::Result<()> {
                use propane::ToSql;
                use propane::prelude::DataObject;
                conn.delete(Self::TABLE, Self::PKCOL, self.pk().to_sql())
            }
        }
        impl propane::ToSql for #tyname {
            fn to_sql(&self) -> propane::SqlVal {
                use propane::DataObject;
                propane::ToSql::to_sql(self.pk())
            }
        }
        impl propane::ToSql for &#tyname {
            fn to_sql(&self) -> propane::SqlVal {
                use propane::DataObject;
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

pub fn impl_dataresult(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let tablelit = make_tablelit(config, tyname);
    let fields_type = fields_type(tyname);
    let numdbfields = fields(&ast_struct).filter(|f| is_row_field(f)).count();
    let rows = rows_for_from(&ast_struct);
    let cols = columns(ast_struct, |_| true);

    let many_init: TokenStream2 =
        fields(&ast_struct)
        .filter(|f| is_many_to_many(f))
        .map(|f| {
            let ident = f
                .ident
                .clone()
                .expect("Fields must be named for propane");
            let many_table_lit = many_table_lit(&ast_struct, f);
            let pksqltype = quote!(<<Self as propane::DataObject>::PKType as propane::FieldType>::SQLTYPE);
            quote!(obj.#ident.ensure_init(#many_table_lit, propane::ToSql::to_sql(obj.pk()), #pksqltype);)
        }).collect();

    quote!(
                impl propane::DataResult for #tyname {
                        type DBO = #tyname;
                        type Fields = #fields_type;
                        const COLUMNS: &'static [propane::db::Column] = &[
                                #cols
                        ];
                        fn from_row(mut row: propane::db::Row) -> propane::Result<Self> {
                                if row.len() != #numdbfields {
                                        return Err(propane::Error::BoundsError(
                                                "Found unexpected number of columns in row for DataResult".to_string()));
                                }
                                let mut it = row.into_iter();
                                let mut obj = #tyname {
                                        state: propane::ObjectState::default(),
                                        #(#rows),*
                                };
                                obj.state.saved = true;
                                #many_init
                                Ok(obj)
                        }
                        fn query() -> propane::query::Query<Self> {
                                propane::query::Query::new(#tablelit)
                        }
                }
    )
}

fn make_tablelit(config: &Config, tyname: &Ident) -> LitStr {
    match &config.table_name {
        Some(s) => make_lit(&s),
        None => make_ident_literal_str(&tyname),
    }
}

pub fn add_fieldexprs(ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let vis = &ast_struct.vis;
    let fieldexprs: Vec<TokenStream2> = fields(ast_struct)
        .map(|f| {
            if is_many_to_many(f) {
                fieldexpr_func_many(f, ast_struct)
            } else {
                fieldexpr_func_regular(f, ast_struct)
            }
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

fn fieldexpr_func_regular(f: &Field, ast_struct: &ItemStruct) -> TokenStream2 {
    let fty = &f.ty;
    let fidlit = field_ident_lit(f);
    fieldexpr_func(
        f,
        ast_struct,
        quote!(propane::query::FieldExpr<#fty>),
        quote!(propane::query::FieldExpr::<#fty>::new(#fidlit)),
    )
}

fn fieldexpr_func_many(f: &Field, ast_struct: &ItemStruct) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let fty = get_foreign_type_argument(&f.ty, "Many").expect("Many field misdetected");
    let many_table_lit = many_table_lit(ast_struct, f);
    fieldexpr_func(
        f,
        ast_struct,
        quote!(propane::query::ManyFieldExpr<#tyname, #fty>),
        quote!(propane::query::ManyFieldExpr::<#tyname, #fty>::new(#many_table_lit)),
    )
}

fn fieldexpr_func(
    f: &Field,
    ast_struct: &ItemStruct,
    field_expr_type: TokenStream2,
    field_expr_ctor: TokenStream2,
) -> TokenStream2 {
    let vis = &ast_struct.vis;
    let fid = match &f.ident {
        Some(fid) => fid,
        None => {
            return quote_spanned!(
                f.span() =>
                    compile_error!("Fields must be named for propane");
            )
        }
    };
    let fnid = Ident::new(&format!("fieldexpr_{}", fid), f.span());
    quote!(
        #vis fn #fnid(&self) -> #field_expr_type {
            #field_expr_ctor
        }
    )
}

fn field_ident_lit(f: &Field) -> TokenStream2 {
    let fid = match &f.ident {
        Some(fid) => fid,
        None => {
            return quote_spanned!(
                f.span() =>
                    compile_error!("Fields must be named for propane");
            )
        }
    };
    make_ident_literal_str(&fid).into_token_stream()
}

fn fields_type(tyname: &Ident) -> Ident {
    Ident::new(&format!("{}Fields", tyname), Span::call_site())
}

fn rows_for_from(ast_struct: &ItemStruct) -> Vec<TokenStream2> {
    fields(&ast_struct)
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            if is_row_field(f) {
                quote!(#ident: propane::FromSql::from_sql(it.next().unwrap())?)
            } else if is_many_to_many(f) {
                quote!(#ident: propane::Many::new())
            } else {
                make_compile_error!(f.span()=> "Unexpected struct field")
            }
        })
        .collect()
}

fn columns<P>(ast_struct: &ItemStruct, mut predicate: P) -> TokenStream2
where
    P: FnMut(&Field) -> bool,
{
    fields(&ast_struct)
        .filter(|f| is_row_field(f) && predicate(f))
        .map(|f| match f.ident.clone() {
            Some(fname) => {
                let ident = make_ident_literal_str(&fname);
                let fty = &f.ty;
                quote!(propane::db::Column::new(#ident, <#fty as propane::FieldType>::SQLTYPE),)
            }
            None => quote_spanned! {
                f.span() =>
                    compile_error!("Fields must be named for propane");
            },
        })
        .collect()
}

fn many_table_lit(ast_struct: &ItemStruct, field: &Field) -> LitStr {
    let tyname = &ast_struct.ident;
    let ident = field
        .ident
        .clone()
        .expect("Fields must be named for propane");
    make_lit(&format!("{}_{}_Many", &tyname, &ident))
}

fn verify_fields(ast_struct: &ItemStruct) -> Option<TokenStream2> {
    let pk_field = pk_field(ast_struct);
    if pk_field.is_none() {
        return Some(make_compile_error!(ast_struct.span() => "No pk field found"));
    };
    let pk_field = pk_field.unwrap();
    for f in fields(ast_struct) {
        if is_auto(f) {
            match get_primitive_sql_type(&f.ty) {
                Some(DeferredSqlType::Known(SqlType::Int)) => (),
                Some(DeferredSqlType::Known(SqlType::BigInt)) => (),
                _ => {
                    return Some(quote_spanned!(
                        f.span() =>
                            compile_error!("Auto is only supported for integer types");
                    ))
                }
            }
            if &pk_field != f {
                return Some(
                    quote_spanned!(f.span() => compile_error!("Auto is currently only supported for the primary key")),
                );
            }
        }
    }
    None
}

fn add_post_insert_for_auto(pk_field: &Field, post_insert: &mut Vec<TokenStream2>) {
    if !is_auto(&pk_field) {
        return;
    }
    let pkident = pk_field.ident.clone().unwrap();
    post_insert.push(quote!(self.#pkident = propane::FromSql::from_sql(pk)?;));
}

/// Builds code for pushing SqlVals for each column satisfying predicate into a vec called `values`
fn push_values<P>(ast_struct: &ItemStruct, mut predicate: P) -> Vec<TokenStream2>
where
    P: FnMut(&Field) -> bool,
{
    fields(&ast_struct)
        .filter(|f| is_row_field(f) && predicate(f))
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            if is_row_field(f) {
                if !is_auto(f) {
                    quote!(values.push(propane::ToSql::to_sql(&self.#ident));)
                } else {
                    quote!()
                }
            } else if is_many_to_many(f) {
                quote!(
                    self.#ident.ensure_init(Self::TABLE, self.pk().clone(), <Self as propane::DataObject>::PKType);
                    self.#ident.save()?;
                )
            } else {
								make_compile_error!(f.span()=> "Unexpected struct field")
            }
        })
        .collect()
}
