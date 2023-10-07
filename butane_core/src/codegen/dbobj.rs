use super::*;
use crate::migrations::adb::{DeferredSqlType, TypeIdentifier};
use crate::SqlType;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Field, ItemStruct};

// Configuration that can be specified with attributes to override default behavior
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

    let insert_cols = columns(ast_struct, |f| !is_auto(f));
    let save_cols = columns(ast_struct, |f| !is_auto(f) && f != &pk_field);

    let mut post_insert: Vec<TokenStream2> = Vec::new();
    add_post_insert_for_auto(&pk_field, &mut post_insert);
    post_insert.push(quote!(self.state.saved = true;));

    let numdbfields = fields(ast_struct).filter(|f| is_row_field(f)).count();
    let many_save: TokenStream2 = fields(ast_struct)
        .filter(|f| is_many_to_many(f))
        .map(|f| {
            let ident = f.ident.clone().expect("Fields must be named for butane");
            let many_table_lit = many_table_lit(ast_struct, f, config);
            let pksqltype =
                quote!(<<Self as butane::DataObject>::PKType as butane::FieldType>::SQLTYPE);
            // Save needs to ensure_initialized
            quote!(
                self.#ident.ensure_init(
                    #many_table_lit,
                    butane::ToSql::to_sql(self.pk()),
                    #pksqltype,
                );
                self.#ident.save(conn)?;
            )
        })
        .collect();

    let fkey_save: TokenStream2 = fields(ast_struct)
        .filter(|f| is_foreign_key(f))
        .map(|f| {
            let ident = f.ident.clone().expect("Fields must be named for butane");

            #[cfg(feature = "auto-save-related")]
            if is_option(f) {
                quote!(
                    if let Some(link_obj) = self.#ident.as_ref() {
                        let rv = link_obj.get();
                        //eprintln!("Saving Option {:?} ?", rv);
                        match rv {
                            Ok(val) => {
                                // ignore failures, as they will be unique constraints
                                let _ = val.clone().save(conn);
                            },
                            Err(_) => {},
                        }
                    }
                )
            } else {
                quote!(
                    let rv = self.#ident.get();
                    //eprintln!("Saving non-Option {:?}", rv);
                    match rv {
                        Ok(val) => {
                            // ignore failures, as they will be unique constraints
                            let _ = val.clone().save(conn);
                        },
                        Err(_) => {},
                    }
                )
            }
            #[cfg(not(feature = "auto-save-related"))]
            quote!(let _ = self.#ident;)
            // quote!(self.#ident.ensure_valpk();)
        })
        .collect();

    let values: Vec<TokenStream2> = push_values(ast_struct, |_| true);
    let values_no_pk: Vec<TokenStream2> = push_values(ast_struct, |f: &Field| f != &pk_field);

    let dataresult = impl_dataresult(ast_struct, tyname, config);
    quote!(
                #dataresult
        impl butane::DataObject for #tyname {
            type PKType = #pktype;
            type Fields = #fields_type;
            const PKCOL: &'static str = #pklit;
            const TABLE: &'static str = #tablelit;
            const AUTO_PK: bool = #auto_pk;
            fn pk(&self) -> &Self::PKType {
                &self.#pkident
            }
            fn save(&mut self, conn: &impl butane::db::ConnectionMethods) -> butane::Result<()> {
                //future perf improvement use an array on the stack
                let mut values: Vec<butane::SqlValRef> = Vec::with_capacity(#numdbfields);
                let pkcol = butane::db::Column::new(
                    #pklit,
                    <#pktype as butane::FieldType>::SQLTYPE);
                if self.state.saved {
                    // Already exists in db, do an update
                    #(#values_no_pk)*
                    if values.len() > 0 {
                        conn.update(
                            Self::TABLE,
                            pkcol,
                            butane::ToSql::to_sql_ref(self.pk()),
                            &[#save_cols],
                            &values,
                        )?;
                    }
                } else if #auto_pk {
                    // Since we expect our pk field to be invalid and to be created by the insert,
                    // we do a pure insert, no upsert allowed.
                    #(#values)*
                    let pk = conn.insert_returning_pk(
                        Self::TABLE,
                        &[#insert_cols],
                        &pkcol,
                        &values,
                    )?;
                    #(#post_insert)*
                } else {
                    // Do an upsert
                    #(#values)*
                    conn.insert_or_replace(Self::TABLE, &[#insert_cols], &pkcol, &values)?;
                    self.state.saved = true
                }
                #fkey_save
                #many_save
                Ok(())
            }
            fn delete(&self, conn: &impl butane::db::ConnectionMethods) -> butane::Result<()> {
                use butane::ToSql;
                use butane::prelude::DataObject;
                conn.delete(Self::TABLE, Self::PKCOL, self.pk().to_sql())
            }

            fn is_saved(&self) -> butane::Result<bool> {
                Ok(self.state.saved)
            }
        }
        impl butane::ToSql for #tyname {
            fn to_sql(&self) -> butane::SqlVal {
                use butane::DataObject;
                butane::ToSql::to_sql(self.pk())
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
                use butane::DataObject;
                butane::ToSql::to_sql_ref(self.pk())
            }
        }
        impl butane::ToSql for &#tyname {
            fn to_sql(&self) -> butane::SqlVal {
                use butane::DataObject;
                butane::ToSql::to_sql(self.pk())
            }
            fn to_sql_ref(&self) -> butane::SqlValRef<'_> {
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
                use butane::DataObject;
                std::borrow::Cow::Borrowed(self.pk())
            }
        }
        impl butane::AsPrimaryKey<#tyname> for &#tyname {
            fn as_pk(&self) -> std::borrow::Cow<<#tyname as butane::DataObject>::PKType> {
                use butane::DataObject;
                std::borrow::Cow::Borrowed(self.pk())
            }
        }
    )
}

/// Code generation to implement the DataResult trait for a model
pub fn impl_dataresult(ast_struct: &ItemStruct, dbo: &Ident, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let numdbfields = fields(ast_struct).filter(|f| is_row_field(f)).count();
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

    /*let fkey_init: TokenStream2 =
        fields(ast_struct)
        .filter(|f| is_foreign_key(f))
        .map(|f| {
            let ident = f
                .ident
                .clone()
                .expect("Fields must be named for butane");
            quote!(let _ = obj.#ident.ensure_valpk();)
        }).collect();
    */
    let dbo_is_self = dbo == tyname;
    let ctor = if dbo_is_self {
        quote!(
            let mut obj = #tyname {
                                state: butane::ObjectState::default(),
                                #(#rows),*
                        };
                        obj.state.saved = true;
        )
    } else {
        quote!(
            let mut obj = #tyname {
                #(#rows),*
            };
        )
    };

    quote!(
                impl butane::DataResult for #tyname {
                        type DBO = #dbo;
                        const COLUMNS: &'static [butane::db::Column] = &[
                                #cols
                        ];
                        fn from_row(mut row: &dyn butane::db::BackendRow) -> butane::Result<Self> {
                                if row.len() != #numdbfields {
                                        return Err(butane::Error::BoundsError(
                                                "Found unexpected number of columns in row for DataResult".to_string()));
                                }
                                #ctor
                                #many_init
                                Ok(obj)
                        }
                    fn query() -> butane::query::Query<Self> {
                        use butane::prelude::DataObject;
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
        quote!(butane::query::FieldExpr<#fty>),
        quote!(butane::query::FieldExpr::<#fty>::new(#fidlit)),
    )
}

fn fieldexpr_func_many(f: &Field, ast_struct: &ItemStruct, config: &Config) -> TokenStream2 {
    let tyname = &ast_struct.ident;
    let fty = get_foreign_type_argument(&f.ty, &MANY_TYNAMES).expect("Many field misdetected");
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
    let mut i: usize = 0;
    fields(ast_struct)
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            if is_row_field(f) {
                let fty = &f.ty;
                let ret = quote!(
                        #ident: butane::FromSql::from_sql_ref(
                                row.get(#i, <#fty as butane::FieldType>::SQLTYPE)?)?
                );
                i += 1;
                ret
            } else if is_many_to_many(f) {
                quote!(#ident: butane::Many::new())
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
    fields(ast_struct)
        .filter(|f| is_row_field(f) && predicate(f))
        .map(|f| match f.ident.clone() {
            Some(fname) => {
                let ident = make_ident_literal_str(&fname);
                let fty = &f.ty;
                quote!(butane::db::Column::new(#ident, <#fty as butane::FieldType>::SQLTYPE),)
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

fn add_post_insert_for_auto(pk_field: &Field, post_insert: &mut Vec<TokenStream2>) {
    if !is_auto(pk_field) {
        return;
    }
    let pkident = pk_field.ident.clone().unwrap();
    post_insert.push(quote!(self.#pkident = butane::FromSql::from_sql(pk)?;));
}

/// Builds code for pushing SqlVals for each column satisfying predicate into a vec called `values`
fn push_values<P>(ast_struct: &ItemStruct, mut predicate: P) -> Vec<TokenStream2>
where
    P: FnMut(&Field) -> bool,
{
    fields(ast_struct)
        .filter(|f| is_row_field(f) && predicate(f))
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            if is_row_field(f) {
                if !is_auto(f) {
                    quote!(values.push(butane::ToSql::to_sql_ref(&self.#ident));)
                } else {
                    quote!()
                }
            } else if is_many_to_many(f) {
                // No-op
                quote!()
            } else {
                make_compile_error!(f.span()=> "Unexpected struct field")
            }
        })
        .collect()
}
