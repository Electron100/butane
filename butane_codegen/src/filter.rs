use butane_core::make_compile_error;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, BinOp, Expr, ExprBinary, ExprMethodCall, ExprPath, Ident, LitStr};

pub fn for_expr(dbres: &Ident, expr: &Expr) -> TokenStream2 {
    handle_expr(&quote!(<#dbres as butane::DataResult>::DBO::fields()), expr)
}

pub fn handle_expr(fields: &impl ToTokens, expr: &Expr) -> TokenStream2 {
    match expr {
        Expr::Binary(binop) => handle_bin_op(fields, binop),
        Expr::MethodCall(mcall) => handle_call(fields, mcall),
        Expr::Path(path) => handle_path(fields, path),
        Expr::Lit(lit) => lit.lit.clone().into_token_stream(),
        Expr::Block(block) => handle_block(&block.block),
        Expr::Group(group) => handle_expr(fields, group.expr.as_ref()),
        _ => {
            let lit = LitStr::new(
                &format!(
                    "Unsupported filter expression '{}' \ndebug info: {:?}",
                    expr.clone().into_token_stream(),
                    expr
                ),
                Span::call_site(),
            );
            quote!(compile_error!(#lit))
        }
    }
}

fn ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

fn handle_block(block: &syn::Block) -> TokenStream2 {
    let stmts = &block.stmts;
    quote!(#(#stmts)*)
}

fn handle_bin_op(fields: &impl ToTokens, binop: &ExprBinary) -> TokenStream2 {
    let left = handle_expr(fields, &binop.left);
    let right = handle_expr(fields, &binop.right);
    match binop.op {
        BinOp::Eq(_) => quote!(#left.eq(&#right)),
        BinOp::Ne(_) => quote!(#left.ne(&#right)),
        BinOp::Lt(_) => quote!(#left.lt(&#right)),
        BinOp::Gt(_) => quote!(#left.gt(&#right)),
        BinOp::Le(_) => quote!(#left.le(&#right)),
        BinOp::Ge(_) => quote!(#left.ge(&#right)),
        BinOp::And(_) => quote!(butane::query::BoolExpr::And(Box::new(#left), Box::new(#right))),
        BinOp::Or(_) => quote!(butane::query::BoolExpr::Or(Box::new(#left), Box::new(#right))),
        _ => quote!(compile_error!("Unsupported binary operator")),
    }
}

fn handle_call(fields: &impl ToTokens, mcall: &ExprMethodCall) -> TokenStream2 {
    let method = mcall.method.to_string();
    match method.as_str() {
        "contains" | "matches" => {
            if mcall.args.len() != 1 {
                return make_compile_error!(mcall.span()=> "expected one argument to '{}'", method);
            };
        }
        _ => (),
    };
    match method.as_str() {
        "matches" => handle_in(fields, &mcall.receiver, mcall.args.first().unwrap()),
        "contains" => handle_contains(fields, &mcall.receiver, mcall.args.first().unwrap()),
        "like" => handle_like(fields, &mcall.receiver, mcall.args.first().unwrap()),
        _ => make_compile_error!("Unknown method call {}", method),
    }
}

fn handle_in(fields: &impl ToTokens, receiver: &Expr, expr: &Expr) -> TokenStream2 {
    let fex = fieldexpr(fields, receiver);
    if let Expr::Lit(lit) = expr {
        // treat this as matching the primary key
        quote!(#fex.subfilterpk(#lit))
    } else {
        // Arbitrary expression
        let q = handle_expr(&quote!(#fex.fields()), expr);
        let span = receiver.span();
        quote_spanned!(span=> #fex.subfilter(#q))
    }
}

fn handle_contains(fields: &impl ToTokens, receiver: &Expr, expr: &Expr) -> TokenStream2 {
    let fex = fieldexpr(fields, receiver);
    if let Expr::Lit(lit) = expr {
        // treat this as matching the primary key
        quote!(#fex.containspk(#lit))
    } else {
        // Arbitrary expression
        let q = handle_expr(&quote!(#fex.fields()), expr);
        let span = receiver.span();
        quote_spanned!(span=> #fex.contains(#q))
    }
}

fn handle_like(fields: &impl ToTokens, receiver: &Expr, expr: &Expr) -> TokenStream2 {
    let fex = fieldexpr(fields, receiver);
    match expr {
        Expr::Binary(_) => make_compile_error!("Unexpected binary expression as parameter to like"),
        Expr::Call(_) => make_compile_error!("Unexpected call expression as parameter to like"),
        _ => {
            // Arbitrary expression
            let q = handle_expr(fields, expr);
            let span = receiver.span();
            quote_spanned!(span=> #fex.like(#q))
        }
    }
}

fn handle_path(fields: &impl ToTokens, expr: &ExprPath) -> TokenStream2 {
    if expr.path.is_ident("None") {
        return quote!(None);
    }
    fieldexpr(fields, &expr.path)
}

fn fieldexpr<F>(fields: &impl ToTokens, field: &F) -> TokenStream2
where
    F: ToTokens + Spanned,
{
    let fieldexpr_ident = ident(&format!("{}", field.into_token_stream()));
    let span = field.span();
    quote_spanned!(span=> #fields.#fieldexpr_ident())
}
