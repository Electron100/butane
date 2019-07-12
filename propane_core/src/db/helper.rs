use super::Column;
use crate::query::BoolExpr::{And, Eq, Ge, Gt, Le, Lt, Ne, Not, Or, Subquery};
use crate::query::Expr;
use crate::query::Expr::{Condition, Placeholder, Val};
use std::fmt::Write;

pub fn sql_for_expr<F, W>(expr: Expr, f: F, w: &mut W)
where
    F: Fn(Expr, &mut W),
    W: Write,
{
    match expr {
        Expr::Column(name) => w.write_str(name),
        Val(v) => w.write_str(&v.to_string()),
        Placeholder => w.write_str("?"),
        Condition(c) => match *c {
            Eq(col, ex) => write!(w, "{} = ", col).and_then(|_| Ok(f(ex, w))),
            Ne(col, ex) => write!(w, "{} <> ", col).and_then(|_| Ok(f(ex, w))),
            Lt(col, ex) => write!(w, "{} < ", col).and_then(|_| Ok(f(ex, w))),
            Gt(col, ex) => write!(w, "{} > ", col).and_then(|_| Ok(f(ex, w))),
            Le(col, ex) => write!(w, "{} <= ", col).and_then(|_| Ok(f(ex, w))),
            Ge(col, ex) => write!(w, "{} >= ", col).and_then(|_| Ok(f(ex, w))),
            And(a, b) => {
                f(Condition(a), w);
                write!(w, " AND ").unwrap();
                f(Condition(b), w);
                Ok(())
            }
            Or(a, b) => {
                f(Condition(a), w);
                write!(w, " OR ").unwrap();
                f(Condition(b), w);
                Ok(())
            }
            Not(a) => write!(w, "NOT ").and_then(|_| Ok(f(Condition(a), w))),
            Subquery(col, tbl2, tbl2_col, ex) => {
                write!(w, "{} IN (SELECT {} FROM {} WHERE ", col, tbl2_col, tbl2).unwrap();
                f(Expr::Condition(ex), w);
                Ok(())
            }
        },
    }
    .unwrap()
}

pub fn sql_select(columns: &[Column], table: &'static str, w: &mut impl Write) {
    let mut colnames: Vec<&'static str> = Vec::new();
    columns.iter().for_each(|c| colnames.push(c.name));
    write!(
        w,
        "SELECT {} from {}",
        colnames.as_slice().join(", "),
        table
    )
    .unwrap()
}

pub fn sql_limit(limit: i32, w: &mut impl Write) {
    write!(w, "LIMIT {}", limit).unwrap();
}
