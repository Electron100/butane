use super::internal::Column;
use crate::query;
use crate::query::Expr::{Condition, Placeholder, Val};
use crate::query::{BoolExpr::*, Expr, Join};
use crate::SqlVal;
use std::fmt::Write;

/// Writes to `w` the SQL to express the expression given in `expr`. Values contained in `expr` are rendered
/// as placeholders in the SQL string and the actual values are added to `values`.
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
        }
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
            Subquery {
                col,
                tbl2,
                tbl2_col,
                expr,
            } => {
                write!(w, "{} IN (SELECT {} FROM {} WHERE ", col, tbl2_col, tbl2).unwrap();
                f(Expr::Condition(expr), values, w);
                write!(w, ")").unwrap();
                Ok(())
            }
            SubqueryJoin {
                col,
                tbl2,
                col2,
                joins,
                expr,
            } => {
                // <col> IN (SELECT <col2> FROM <tbl2> <joins> WHERE <expr>)
                write!(w, "{} IN (SELECT ", col).unwrap();
                sql_column(col2, w);
                write!(w, " FROM {} ", tbl2).unwrap();
                sql_joins(joins, w);
                write!(w, " WHERE ").unwrap();
                f(Expr::Condition(expr), values, w);
                write!(w, ")").unwrap();
                Ok(())
            }
            In(col, vals) => write!(
                w,
                "{} IN ({})",
                col,
                vals.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .as_slice()
                    .join(", ")
            ),
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

pub fn sql_limit(limit: i32, w: &mut impl Write) {
    write!(w, " LIMIT {}", limit).unwrap();
}

fn list_columns(columns: &[Column], w: &mut impl Write) {
    let mut colnames: Vec<&'static str> = Vec::new();
    columns.iter().for_each(|c| colnames.push(c.name()));
    write!(w, "{}", colnames.as_slice().join(",")).unwrap();
}

fn sql_joins(joins: Vec<Join>, w: &mut impl Write) {
    for join in joins {
        match join {
            Join::Inner {
                join_table,
                col1,
                col2,
            } => {
                // INNER JOIN <join_table> ON <col1> = <col2>
                write!(w, "INNER JOIN {} ON ", join_table).unwrap();
                sql_column(col1, w);
                w.write_str(" = ").unwrap();
                sql_column(col2, w);
            }
        }
    }
}

fn sql_column(col: query::Column, w: &mut impl Write) {
    match col.table() {
        Some(table) => write!(w, "{}.{}", table, col.name()),
        None => w.write_str(col.name()),
    }
    .unwrap()
}
