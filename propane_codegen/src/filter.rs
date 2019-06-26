use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned, ToTokens};
use syn;
use syn::{BinOp, Expr, ExprBinary, ExprMethodCall, ExprPath, Field, Ident, ItemStruct, LitStr};

pub fn for_expr(dbobj: &Ident, expr: &Expr) -> TokenStream2 {
    eprintln!("Expr is {:?}", expr);
    match expr {
        Expr::Binary(binop) => handle_bin_op(dbobj, binop),
        Expr::MethodCall(mcall) => handle_call(dbobj, mcall),
        Expr::Path(path) => handle_path(dbobj, path),
        Expr::Lit(lit) => lit.lit.clone().into_token_stream(),
        _ => {
            let lit = LitStr::new(
                &format!(
                    "Unsupported filter expression '{}'",
                    expr.clone().into_token_stream(),
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

fn handle_bin_op(dbobj: &Ident, binop: &ExprBinary) -> TokenStream2 {
    let left = for_expr(dbobj, &binop.left);
    let right = for_expr(dbobj, &binop.right);
    match binop.op {
        BinOp::Eq(_) => quote!(#left.eq(#right)),
        BinOp::Ne(_) => quote!(#left.ne(#right)),
        BinOp::Lt(_) => quote!(#left.lt(#right)),
        BinOp::Gt(_) => quote!(#left.gt(#right)),
        BinOp::Le(_) => quote!(#left.le(#right)),
        BinOp::Ge(_) => quote!(#left.ge(#right)),
        BinOp::And(_) => quote!(propane::query::BoolExpr::And(Box::new(#left), Box::new(#right))),
        BinOp::Or(_) => quote!(propane::query::BoolExpr::Or(Box::new(#left), Box::new(#right))),
        _ => quote!(compile_error!("Unsupported binary operator")),
    }
}

fn handle_call(dbobj: &Ident, mcall: &ExprMethodCall) -> TokenStream2 {
    quote!(compile_error!("TODO support method call"))
}

fn handle_path(dbobj: &Ident, expr: &ExprPath) -> TokenStream2 {
    let field = &expr.path;
    quote!(#dbobj::fieldexpr_#field())
}
