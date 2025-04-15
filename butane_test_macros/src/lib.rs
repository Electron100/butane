//! Macros for butane tests.

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{ext::IdentExt, parse_macro_input, punctuated::Punctuated, Ident, ItemFn, Stmt, Token};

/// Create a SQLite and PostgreSQL `#[test]` that each invoke `$fname` with a `Connection` with no schema.
#[proc_macro_attribute]
pub fn butane_test(args: TokenStream, input: TokenStream) -> TokenStream {
    let input: TokenStream2 = input.into();
    let func: ItemFn = syn::parse2(input.clone()).unwrap();
    let fname = func.sig.ident.to_string();

    // Handle arguments
    let options: Vec<TestOption> =
        parse_macro_input!(args with Punctuated::<TestOption, Token![,]>::parse_terminated)
            .into_iter()
            .collect();
    let include_sync = !options.contains(&TestOption::Async);
    let include_async = !options.contains(&TestOption::Sync);
    let migrate = !options.contains(&TestOption::NoMigrate);

    let mut func_sync = func.clone();

    // Using butane_core rather than butane::prelude because the butane_test macro is used for butane_core tests too
    let sync_prelude: Stmts = syn::parse2(quote!(
        use butane_core::DataObject;
        use butane_core::DataResult;
        use butane_core::db::BackendConnection;
        use butane_core::fkey::ForeignKeyOpsSync;
        use butane_core::many::ManyOpsSync;
        use butane_core::query::QueryOpsSync;
        use butane_core::DataObjectOpsSync;
    ))
    .unwrap();

    func_sync.block.stmts = sync_prelude
        .into_iter()
        .chain(func_sync.block.stmts)
        .collect();

    let async_prelude: Stmts = syn::parse2(quote!(
        use butane_core::DataObject;
        use butane_core::DataResult;
        use butane_core::db::BackendConnectionAsync;
        use butane_core::fkey::ForeignKeyOpsAsync;
        use butane_core::many::ManyOpsAsync;
        use butane_core::query::QueryOpsAsync;
        use butane_core::DataObjectOpsAsync;
    ))
    .unwrap();

    let mut func_async = func;
    func_async.block.stmts = async_prelude
        .into_iter()
        .chain(func_async.block.stmts)
        .collect();

    let mut funcs = Vec::<TokenStream2>::new();
    if include_sync {
        funcs.push(quote!(
            #[maybe_async_cfg::maybe(
                sync(),
                idents(
                    ConnectionAsync(sync="Connection"),
                    find_async(sync="find"),
                    setup_blog(sync="setup_blog_sync"),
                    create_tag(sync="create_tag_sync"),
                )
            )]
            #[cfg(test)]
            #func_sync
        ));
    }
    if include_async {
        funcs.push(quote!(
            #[maybe_async_cfg::maybe(async())]
            #[cfg(test)]
            #func_async
        ));
    }

    let mut backends: Vec<(&'static str, &'static str)> = Vec::new();
    backends.push(("pg", "PgTestInstance"));
    if !options.contains(&TestOption::PgOnly) {
        backends.push(("sqlite", "SQLiteTestInstance"));
    }

    let tests = backends
        .into_iter()
        .map(|b| make_tests(&fname, b.0, b.1, include_sync, include_async, migrate));

    quote! {
        #(#funcs)*
        #(#tests)*
    }
    .into()
}

// Make both sync and async tests
fn make_tests(
    fname_base: &str,
    backend_name: &str,
    instance_name: &str,
    include_sync: bool,
    include_async: bool,
    migrate: bool,
) -> TokenStream2 {
    if include_sync {
        let mut tstream = make_sync_test(fname_base, backend_name, instance_name, migrate);
        if include_async {
            tstream.extend([make_async_test(
                fname_base,
                backend_name,
                instance_name,
                migrate,
            )]);
        }
        tstream
    } else if include_async {
        make_async_test(fname_base, backend_name, instance_name, migrate)
    } else {
        panic!("Either sync or async must be supported")
    }
}

fn make_async_test(
    fname_base: &str,
    backend_name: &str,
    instance_name: &str,
    migrate: bool,
) -> TokenStream2 {
    let fname_full = make_ident(&format!("{fname_base}_async_{backend_name}"));
    let fname_async = make_ident(&format!("{fname_base}_async"));
    let instance_ident = make_ident(instance_name);
    quote! {
        cfg_if::cfg_if! {
            if #[cfg(feature = #backend_name)] {
                #[tokio::test]
                pub async fn #fname_full () {
                    use butane_test_helper::*;
                    #instance_ident::run_test_async(#fname_async, #migrate).await;
                }
            }
        }
    }
}

fn make_sync_test(
    fname_base: &str,
    backend_name: &str,
    instance_name: &str,
    migrate: bool,
) -> TokenStream2 {
    let fname_full = Ident::new(
        &format!("{fname_base}_sync_{backend_name}"),
        Span::call_site(),
    );
    let fname_sync = Ident::new(&format!("{fname_base}_sync"), Span::call_site());
    let instance_ident = Ident::new(instance_name, Span::call_site());
    quote! {
        cfg_if::cfg_if! {
            if #[cfg(feature = #backend_name)] {
                #[test]
                pub fn #fname_full () {
                    use butane_test_helper::*;
                    #instance_ident::run_test_sync(#fname_sync, #migrate);
                }
            }
        }
    }
}

fn make_ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

/// Options for butane_test.
#[derive(PartialEq, Eq)]
enum TestOption {
    Sync,
    Async,
    NoMigrate,
    PgOnly,
}

impl Parse for TestOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(<Ident as IdentExt>::peek_any) {
            let name: Ident = input.call(IdentExt::parse_any)?;
            if name == "async" {
                Ok(TestOption::Async)
            } else if name == "sync" {
                Ok(TestOption::Sync)
            } else if name == "nomigrate" {
                Ok(TestOption::NoMigrate)
            } else if name == "pg" {
                Ok(TestOption::PgOnly)
            } else {
                Err(syn::Error::new(
                    name.span(),
                    "Unknown option for butane_test",
                ))
            }
        } else {
            Err(lookahead.error())
        }
    }
}

struct Stmts {
    stmts: Vec<Stmt>,
}

impl Parse for Stmts {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut stmts = Vec::new();
        while !input.is_empty() {
            stmts.push(input.parse()?);
        }
        Ok(Self { stmts })
    }
}

impl From<Stmts> for Vec<Stmt> {
    fn from(stmts: Stmts) -> Vec<Stmt> {
        stmts.stmts
    }
}

impl IntoIterator for Stmts {
    type Item = Stmt;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.stmts.into_iter()
    }
}
