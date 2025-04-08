use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, Attribute, Field, ItemStruct, LitStr};

use super::{
    fields, get_autopk_sql_type, get_type_argument, is_auto, is_many_to_many, is_row_field,
    make_ident_literal_str, make_lit, pk_field, MANY_TYNAMES,
};
use crate::migrations::adb::{DeferredSqlType, TypeIdentifier, MANY_SUFFIX};
use crate::SqlType;

/// Configuration that can be specified with attributes to override default behavior
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub table_name: Option<String>,
}

/// Code generation to implement the DataObject trait for a model
pub fn impl_dbobject(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let tablelit = make_tablelit(config, tyname);
    let fields_type = fields_type(tyname);

    let err = verify_fields(ast_struct);
    if let Some(err) = err {
        return err;
    }

    let pk_field = pk_field(ast_struct).unwrap();
    let pktype = &pk_field.ty;
    let pkident = pk_field.ident.clone().unwrap();
    let pklit = make_ident_literal_str(&pkident);
    let auto_pk = is_auto(&pk_field);

    let values: Vec<TokenStream2> = push_values(ast_struct, |_| true);
    let values_no_pk: Vec<TokenStream2> = push_values(ast_struct, |f: &Field| f != &pk_field);
    let insert_cols = columns(ast_struct, |f| !is_auto(f));

    let many_save_sync = impl_many_save(ast_struct, config, false);
    let save_many_to_many_async = def_for_save_many_to_many_async(ast_struct, config);

    let conn_arg_name = if many_save_sync.is_empty() {
        syn::Ident::new("_conn", Span::call_site())
    } else {
        syn::Ident::new("conn", Span::call_site())
    };

    let non_auto_values_fn = if values.is_empty() {
        quote!(
            fn non_auto_values(&self, _include_pk: bool) -> Vec<butane::SqlValRef> {
                return vec![];
            }
        )
    } else {
        quote!(
            fn non_auto_values(&self, include_pk: bool) -> Vec<butane::SqlValRef> {
                let mut values: Vec<butane::SqlValRef> = Vec::with_capacity(
                    <Self as butane::DataResult>::COLUMNS.len()
                );
                if include_pk {
                    #(#values)*
                } else {
                    #(#values_no_pk)*
                }
                values
            }
        )
    };

    let dataresult = impl_dataresult(ast_struct, tyname, config);
    // Note the many impls following DataObject can not be generic because they implement for T and &T,
    // which become conflicting types as &T is included in T.
    // https://stackoverflow.com/questions/66241700
    quote!(
        #dataresult

        impl butane::internal::DataObjectInternal for #tyname {
            const NON_AUTO_COLUMNS: &'static [butane::db::Column] = &[
                #insert_cols
            ];

            fn pk_mut(&mut self) -> &mut impl butane::PrimaryKeyType {
                &mut self.#pkident
            }
            #save_many_to_many_async
            fn save_many_to_many_sync(
                &mut self,
                #conn_arg_name: &impl butane::db::ConnectionMethods,
            ) -> butane::Result<()> {
                #many_save_sync
                Ok(())
            }
            #non_auto_values_fn
        }

        impl butane::DataObject for #tyname {
            type PKType = #pktype;
            type Fields = #fields_type;
            const PKCOL: &'static str = #pklit;
            const TABLE: &'static str = #tablelit;
            const AUTO_PK: bool = #auto_pk;

            fn pk(&self) -> &Self::PKType {
                &self.#pkident
            }
        }
        impl butane::ToSql for #tyname {
            fn to_sql(&self) -> butane::SqlVal {
                #[allow(unused_imports)]
                use butane::DataObject;
                butane::ToSql::to_sql(self.pk())
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
                #[allow(unused_imports)]
                use butane::DataObject;
                butane::ToSql::to_sql_ref(self.pk())
            }
        }
        impl butane::ToSql for &#tyname {
            fn to_sql(&self) -> butane::SqlVal {
                #[allow(unused_imports)]
                use butane::DataObject;
                butane::ToSql::to_sql(self.pk())
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
                #[allow(unused_imports)]
                use butane::DataObject;
                butane::ToSql::to_sql_ref(self.pk())
            }
        }
        impl PartialEq<butane::ForeignKey<#tyname>> for #tyname {
            fn eq(&self, other: &butane::ForeignKey<#tyname>) -> bool {
                other.eq(&self)
            }
        }
        impl PartialEq<butane::ForeignKey<#tyname>> for &#tyname {
            fn eq(&self, other: &butane::ForeignKey<#tyname>) -> bool {
                other.eq(self)
            }
        }
        impl butane::AsPrimaryKey<#tyname> for #tyname {
            fn as_pk(&self) -> std::borrow::Cow<<Self as butane::DataObject>::PKType> {
                #[allow(unused_imports)]
                use butane::DataObject;
                std::borrow::Cow::Borrowed(self.pk())
            }
        }
        impl butane::AsPrimaryKey<#tyname> for &#tyname {
            fn as_pk(&self) -> std::borrow::Cow<<#tyname as butane::DataObject>::PKType> {
                #[allow(unused_imports)]
                use butane::DataObject;
                std::borrow::Cow::Borrowed(self.pk())
            }
        }
    )
}

/// Code generation to implement the DataResult trait for a model
pub fn impl_dataresult(ast_struct: &ItemStruct, dbo: &Ident, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let rows = rows_for_from(ast_struct);
    let cols = columns(ast_struct, |_| true);

    let many_init: TokenStream2 = fields(ast_struct)
        .filter(|f| is_many_to_many(f))
        .map(|f| {
            let ident = f.ident.clone().expect("Fields must be named for butane");
            let many_table_lit = many_table_lit(ast_struct, f, config);
            let pksqltype =
                quote!(<<Self as butane::DataObject>::PKType as butane::FieldType>::SQLTYPE);
            quote!(
                obj.#ident.ensure_init(
                    #many_table_lit,
                    butane::ToSql::to_sql(obj.pk()),
                    #pksqltype,
                );
            )
        })
        .collect();

    let from_row_body = if many_init.is_empty() {
        quote!(
            Ok(#tyname {
                #(#rows),*
            })
        )
    } else {
        quote!(
            let mut obj = #tyname {
                #(#rows),*
            };
            #many_init
            Ok(obj)
        )
    };

    quote!(
        impl butane::DataResult for #tyname {
            type DBO = #dbo;
            const COLUMNS: &'static [butane::db::Column] = &[
                #cols
            ];
            fn from_row(row: &dyn butane::db::BackendRow) -> butane::Result<Self> {
                use butane::DataObject;
                if row.len() != Self::COLUMNS.len() {
                    return Err(butane::Error::BoundsError(
                        "Found unexpected number of columns in row for DataResult".to_string()
                    ));
                }
                let mut i = 0;
                #from_row_body
            }
            fn query() -> butane::query::Query<Self> {
                #[allow(unused_imports)]
                use butane::DataObject;
                butane::query::Query::new(Self::DBO::TABLE)
            }
        }
    )
}

fn make_tablelit(config: &Config, tyname: &Ident) -> LitStr {
    match &config.table_name {
        Some(s) => make_lit(s),
        None => make_ident_literal_str(tyname),
    }
}

/// Help to generate field expressions for each `#[butane::model]`.
pub fn add_fieldexprs(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
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
    quote!(
        impl #tyname {
            /// Get fields.
            pub fn fields() -> #fields_type {
                #fields_type::default()
            }
        }
        /// Helper struct for butane model.
        #vis struct #fields_type;
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
    let fid = match &f.ident {
        Some(fid) => fid,
        None => {
            return quote_spanned!(
                f.span() =>
                    compile_error!("Fields must be named for butane");
            )
        }
    };
    let fnid = Ident::new(&format!("{fid}"), f.span());
    let cfg_attr = cfg_attrs(&f.attrs);
    quote!(
        /// Create query expression.
        #(#cfg_attr)*
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
                    compile_error!("Fields must be named for butane");
            )
        }
    };
    make_ident_literal_str(fid).into_token_stream()
}

fn fields_type(tyname: &Ident) -> Ident {
    Ident::new(&format!("{tyname}Fields"), Span::call_site())
}

fn rows_for_from(ast_struct: &ItemStruct) -> Vec<TokenStream2> {
    fields(ast_struct)
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            let cfg_attrs = cfg_attrs(&f.attrs);
            if is_row_field(f) {
                let ret = quote!(
                    #(#cfg_attrs)*
                    #ident: {
                        let value = butane::FromSql::from_sql_ref(
                            row.get(i, Self::COLUMNS[i].ty().clone())?
                        )?;
                        i += 1;
                        value
                    }
                );
                ret
            } else if is_many_to_many(f) {
                quote!(
                    #(#cfg_attrs)*
                    #ident: butane::Many::new()
                )
            } else {
                make_compile_error!(f.span()=> "Unexpected struct field")
            }
        })
        .collect()
}

fn cfg_attrs(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect()
}

fn columns<P>(ast_struct: &ItemStruct, mut predicate: P) -> TokenStream2
where
    P: FnMut(&Field) -> bool,
{
    fields(ast_struct)
        .filter(|f| is_row_field(f) && predicate(f))
        .map(|f| match f.ident.clone() {
            Some(fname) => {
                let ident = make_ident_literal_str(&fname);
                let fty = &f.ty;
                let attrs = cfg_attrs(&f.attrs);
                quote!(
                    #(#attrs)*
                    butane::db::Column::new(#ident, <#fty as butane::FieldType>::SQLTYPE),
                )
            }
            None => quote_spanned! {
                f.span() =>
                    compile_error!("Fields must be named for butane");
            },
        })
        .collect()
}

fn many_table_lit(ast_struct: &ItemStruct, field: &Field, config: &Config) -> LitStr {
    let ident = field
        .ident
        .clone()
        .expect("Fields must be named for butane");
    let binding = ast_struct.ident.to_string();
    let tyname = match &config.table_name {
        Some(s) => s,
        None => &binding,
    };
    make_lit(&format!("{}_{}{MANY_SUFFIX}", &tyname, &ident))
}

fn verify_fields(ast_struct: &ItemStruct) -> Option<TokenStream2> {
    let pk_field = pk_field(ast_struct);
    if pk_field.is_none() {
        return Some(make_compile_error!(ast_struct.span() => "No pk field found"));
    };
    let pk_field = pk_field.unwrap();
    for f in fields(ast_struct) {
        if is_auto(f) {
            match get_autopk_sql_type(&f.ty) {
                Some(DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int))) => (),
                Some(DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::BigInt))) => (),
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

/// Builds code for pushing SqlVals for each column satisfying predicate into a vec called `values`
/// that excludes any auto values.
fn push_values<P>(ast_struct: &ItemStruct, mut predicate: P) -> Vec<TokenStream2>
where
    P: FnMut(&Field) -> bool,
{
    fields(ast_struct)
        .filter(|f| is_row_field(f) && !is_auto(f) && predicate(f))
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            let cfg_attrs = cfg_attrs(&f.attrs);
            quote!(
                #(#cfg_attrs)*
                values.push(butane::ToSql::to_sql_ref(&self.#ident));
            )
        })
        .collect()
}

fn impl_many_save(ast_struct: &ItemStruct, config: &Config, is_async: bool) -> TokenStream2 {
    fields(ast_struct)
        .filter(|f| is_many_to_many(f))
        .map(|f| {
            let ident = f.ident.clone().expect("Fields must be named for butane");
            let many_table_lit = many_table_lit(ast_struct, f, config);
            let pksqltype =
                quote!(<<Self as butane::DataObject>::PKType as butane::FieldType>::SQLTYPE);

            let save_with_conn = if is_async {
                quote!(butane::ManyOpsAsync::save(&mut self.#ident, conn).await?;)
            } else {
                quote!(butane::ManyOpsSync::save(&mut self.#ident, conn)?;)
            };

            // Save needs to ensure_initialized
            quote!(
                self.#ident.ensure_init(
                    #many_table_lit,
                    butane::ToSql::to_sql(butane::DataObject::pk(self)),
                    #pksqltype,
                );
                #save_with_conn
            )
        })
        .collect()
}

#[cfg(feature = "async")]
fn def_for_save_many_to_many_async(ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let many_save_async = impl_many_save(ast_struct, config, true);
    let conn_arg_name = if many_save_async.is_empty() {
        syn::Ident::new("_conn", Span::call_site())
    } else {
        syn::Ident::new("conn", Span::call_site())
    };

    quote!(
        async fn save_many_to_many_async(
            &mut self,
            #conn_arg_name: &impl butane::db::ConnectionMethodsAsync,
        ) -> butane::Result<()> {
            #many_save_async
            Ok(())
        }
    )
}

#[cfg(not(feature = "async"))]
fn def_for_save_many_to_many_async(_ast_struct: &ItemStruct, _config: &Config) -> TokenStream2 {
    quote!()
}
