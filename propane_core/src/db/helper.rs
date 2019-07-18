use super::Column;
use crate::query::BoolExpr::{And, Eq, Ge, Gt, Le, Lt, Ne, Not, Or, Subquery};
use crate::query::Expr;
use crate::query::Expr::{Condition, Placeholder, Val};
use crate::SqlVal;
use std::fmt::Write;

pub fn sql_for_expr<F, W>(expr: Expr, f: F, values: &mut Vec<SqlVal>, w: &mut W)
where
    F: Fn(Expr, &mut Vec<SqlVal>, &mut W),
    W: Write,
{
    match expr {
        Expr::Column(name) => w.write_str(name),
        Val(v) => {
            values.push(v);
            w.write_str("?")
        },
        Placeholder => w.write_str("?"),
        Condition(c) => match *c {
            Eq(col, ex) => write!(w, "{} = ", col).and_then(|_| Ok(f(ex, values, w))),
            Ne(col, ex) => write!(w, "{} <> ", col).and_then(|_| Ok(f(ex, values, w))),
            Lt(col, ex) => write!(w, "{} < ", col).and_then(|_| Ok(f(ex, values, w))),
            Gt(col, ex) => write!(w, "{} > ", col).and_then(|_| Ok(f(ex, values, w))),
            Le(col, ex) => write!(w, "{} <= ", col).and_then(|_| Ok(f(ex, values, w))),
            Ge(col, ex) => write!(w, "{} >= ", col).and_then(|_| Ok(f(ex, values, w))),
            And(a, b) => {
                f(Condition(a), values, w);
                write!(w, " AND ").unwrap();
                f(Condition(b), values, w);
                Ok(())
            }
            Or(a, b) => {
                f(Condition(a), values, w);
                write!(w, " OR ").unwrap();
                f(Condition(b), values, w);
                Ok(())
            }
            Not(a) => write!(w, "NOT ").and_then(|_| Ok(f(Condition(a), values, w))),
            Subquery(col, tbl2, tbl2_col, ex) => {
                write!(w, "{} IN (SELECT {} FROM {} WHERE ", col, tbl2_col, tbl2).unwrap();
                f(Expr::Condition(ex), values, w);
                Ok(())
            }
        },
    }
    .unwrap()
}

pub fn sql_select(columns: &[Column], table: &'static str, w: &mut impl Write) {
    write!(w, "SELECT ").unwrap();
    list_columns(columns, w);
    write!(w, " FROM {}", table).unwrap();
}

pub fn sql_insert_or_replace_with_placeholders(
    table: &'static str,
    columns: &[Column],
    w: &mut impl Write,
) {
    write!(w, "INSERT OR REPLACE INTO {} (", table).unwrap();
    list_columns(columns, w);
    write!(w, ") VALUES (").unwrap();
    columns.iter().fold("", |sep, _| {
        write!(w, "{}?", sep).unwrap();
        ", "
    });
    write!(w, ")").unwrap();
}

pub fn sql_delete_with_placeholder(table: &'static str, pkcol: &'static str, w: &mut impl Write) {
    write!(w, "DELETE FROM {} WHERE {} = ?", table, pkcol).unwrap();
}

pub fn sql_limit(limit: i32, w: &mut impl Write) {
    write!(w, " LIMIT {}", limit).unwrap();
}

fn list_columns(columns: &[Column], w: &mut impl Write) {
    let mut colnames: Vec<&'static str> = Vec::new();
    columns.iter().for_each(|c| colnames.push(c.name));
    write!(w, "{}", colnames.as_slice().join(",")).unwrap();
}
