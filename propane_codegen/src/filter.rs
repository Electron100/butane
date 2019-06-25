use syn::ExprBinary;
use proc_macro2::TokenStream as TokenStream2;
use syn::{Expr, ExprBinary, ExprMethodCall, Field, ItemStruct, LitStr};
use quote::{quote, quote_spanned, ToTokens};

pub fn expr_for(expr: &Expr) -> TokenStream2 {
    match expr {
        Expr::Binary(binop) => handle_bin_op(binop),
        Expr::MethodCall(mcall) => handle_call(mcall),
        _ => {
            let lit = LitStr::new(&format!("Unsupported filter expression", expr), Span::call_site())
            quote!(compile_error!("Unsupported filter expression: "))
        }
    }
}

fn ident(name: &str) {
    Ident::new(name, Span::call_site())
}

fn handle_bin_op(binop: &ExprBinary) -> TokenStream2 {
    let left = expr_for(binop.left);
    let right = expr_for(binop.right);
    match binop.op {
        BinOp::Eq(_) => quote!(#left.eq(#right)),
        BinOp::Ne(_) => quote!(#left.ne(#right)),
        BinOp::Lt(_) => quote!(#left.lt(#right)),
        BinOp::Gt(_) => quote!(#left.gt(#right)),
        BinOp::Le(_) => quote!(#left.le(#right)),
        BinOp::Ge(_) => quote!(#left.ge(#right)),
        BinOp::And(_) => quote!(propane::query::BoolExpr::And(Box::new(#left), Box::new(#right))),
        BinOp::Or(_) => quote!(propane::query::BoolExpr::Or(Box::new(#left), Box::new(#right))),
        _ => quote!(compile_error!(""))
    }
}

fn handle_call(mcall: &ExprMethodCall) -> TokenStream2 {

}