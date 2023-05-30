#![allow(clippy::many_single_char_names)]
#![allow(clippy::unit_arg)]
// may occur if no backends are selected
#![allow(unused)]

use super::Column;
use crate::migrations::adb::{AColumn, TypeIdentifier};
use crate::query::Expr::{Condition, Placeholder, Val};
use crate::query::{BoolExpr::*, Expr, Join, Order, OrderDirection};
use crate::Error;
use crate::{query, Result, SqlType, SqlVal};
use std::borrow::Cow;
use std::fmt::Write;

#[cfg(feature = "datetime")]
use chrono::naive::NaiveDateTime;

pub trait PlaceholderSource {
    fn next_placeholder(&mut self) -> Cow<str>;
}

/// Quotes the `word` if it is a reserved word.
pub fn quote_reserved_word(word: &str) -> Cow<str> {
    if sqlparser::keywords::ALL_KEYWORDS.contains(&word.to_uppercase().as_str()) {
        format!("\"{}\"", word).into()
    } else {
        word.into()
    }
}

/// Writes to `w` the SQL to express the expression given in `expr`. Values contained in `expr` are rendered
/// as placeholders in the SQL string and the actual values are added to `values`.
pub fn sql_for_expr<F, P, W>(expr: Expr, f: F, values: &mut Vec<SqlVal>, pls: &mut P, w: &mut W)
where
    F: Fn(Expr, &mut Vec<SqlVal>, &mut P, &mut W),
    P: PlaceholderSource,
    W: Write,
{
    match expr {
        Expr::Column(name) => w.write_str(&quote_reserved_word(name)),
        Val(v) => match v {
            // No risk of SQL injection with integers and the
            // different sizes are tricky with the PG backend's binary
            // protocol
            SqlVal::Int(i) => write!(w, "{i}"),
            SqlVal::BigInt(i) => write!(w, "{i}"),
            _ => {
                values.push(v);
                w.write_str(&pls.next_placeholder())
            }
        },
        Placeholder => w.write_str(&pls.next_placeholder()),
        Condition(c) => match *c {
            True => write!(w, "TRUE"),
            Eq(col, ex) => match ex {
                Expr::Val(SqlVal::Null) => write!(w, "{col} IS NULL"),
                _ => write!(w, "{col} = ").and_then(|_| Ok(f(ex, values, pls, w))),
            },
            Ne(col, ex) => match ex {
                Expr::Val(SqlVal::Null) => write!(w, "{col} IS NOT NULL"),
                _ => write!(w, "{col} <> ").and_then(|_| Ok(f(ex, values, pls, w))),
            },
            Lt(col, ex) => write!(w, "{col} < ").and_then(|_| Ok(f(ex, values, pls, w))),
            Gt(col, ex) => write!(w, "{col} > ").and_then(|_| Ok(f(ex, values, pls, w))),
            Le(col, ex) => write!(w, "{col} <= ").and_then(|_| Ok(f(ex, values, pls, w))),
            Ge(col, ex) => write!(w, "{col} >= ").and_then(|_| Ok(f(ex, values, pls, w))),
            Like(col, ex) => write!(w, "{col} like ").and_then(|_| Ok(f(ex, values, pls, w))),
            AllOf(conds) => {
                let mut remaining = conds.len();
                for cond in conds {
                    // future perf improvement: figure out a clean way to avoid the extra boxing
                    f(Condition(Box::new(cond)), values, pls, w);
                    if remaining > 1 {
                        write!(w, " AND ").unwrap();
                        remaining -= 1;
                    }
                }
                Ok(())
            }
            And(a, b) => {
                f(Condition(a), values, pls, w);
                write!(w, " AND ").unwrap();
                f(Condition(b), values, pls, w);
                Ok(())
            }
            Or(a, b) => {
                f(Condition(a), values, pls, w);
                write!(w, " OR ").unwrap();
                f(Condition(b), values, pls, w);
                Ok(())
            }
            Not(a) => write!(w, "NOT ").and_then(|_| Ok(f(Condition(a), values, pls, w))),
            Subquery {
                col,
                tbl2,
                tbl2_col,
                expr,
            } => {
                write!(
                    w,
                    "{} IN (SELECT {} FROM {} WHERE ",
                    quote_reserved_word(col),
                    quote_reserved_word(tbl2_col),
                    quote_reserved_word(&tbl2),
                )
                .unwrap();
                f(Expr::Condition(expr), values, pls, w);
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
                write!(w, "{} IN (SELECT ", quote_reserved_word(col)).unwrap();
                sql_column(col2, w);
                write!(w, " FROM {} ", quote_reserved_word(&tbl2)).unwrap();
                sql_joins(joins, w);
                write!(w, " WHERE ").unwrap();
                f(Expr::Condition(expr), values, pls, w);
                write!(w, ")").unwrap();
                Ok(())
            }
            In(col, vals) => {
                write!(w, "{} IN (", quote_reserved_word(col)).unwrap();
                let mut remaining = vals.len();
                for val in vals {
                    f(Expr::Val(val), values, pls, w);
                    if remaining > 1 {
                        write!(w, ", ").unwrap();
                        remaining -= 1;
                    }
                }
                write!(w, ")")
            }
        },
    }
    .unwrap()
}

pub fn sql_select(columns: &[Column], table: &str, w: &mut impl Write) {
    write!(w, "SELECT ").unwrap();
    list_columns(columns, w);
    write!(w, " FROM {table}").unwrap();
}

pub fn sql_insert_with_placeholders(
    table: &str,
    columns: &[Column],
    pls: &mut impl PlaceholderSource,
    w: &mut impl Write,
) {
    write!(w, "INSERT INTO {} ", quote_reserved_word(table)).unwrap();
    if !columns.is_empty() {
        write!(w, "(").unwrap();
        list_columns(columns, w);
        write!(w, ") VALUES (").unwrap();
        columns.iter().fold("", |sep, _| {
            write!(w, "{}{}", sep, pls.next_placeholder()).unwrap();
            ", "
        });
        write!(w, ")").unwrap();
    } else {
        write!(w, "DEFAULT VALUES ").unwrap();
    }
}

/// Writes to `w` the SQL of an UPDATE to `table` of `columns` using values in `pls`,
/// for the row uniquely identified by `pkcol`.
pub fn sql_update_with_placeholders(
    table: &str,
    pkcol: Column,
    columns: &[Column],
    pls: &mut impl PlaceholderSource,
    w: &mut impl Write,
) {
    write!(w, "UPDATE {} SET ", quote_reserved_word(table)).unwrap();
    columns.iter().fold("", |sep, c| {
        write!(
            w,
            "{}{} = {}",
            sep,
            quote_reserved_word(c.name()),
            pls.next_placeholder()
        )
        .unwrap();
        ", "
    });
    write!(
        w,
        " WHERE {} = {}",
        quote_reserved_word(pkcol.name()),
        pls.next_placeholder()
    )
    .unwrap();
}

pub fn sql_limit(limit: i32, w: &mut impl Write) {
    write!(w, " LIMIT {limit}").unwrap();
}

pub fn sql_offset(offset: i32, w: &mut impl Write) {
    write!(w, " OFFSET {offset}").unwrap();
}

/// Writes to `w` the SQL of the list of column `order`.
pub fn sql_order(order: &[Order], w: &mut impl Write) {
    write!(w, " ORDER BY ").unwrap();
    order.iter().fold("", |sep, o| {
        let sql_dir = match o.direction {
            OrderDirection::Ascending => "ASC",
            OrderDirection::Descending => "DESC",
        };
        write!(w, "{}{} {}", sep, quote_reserved_word(o.column), sql_dir).unwrap();
        ", "
    });
}

pub fn column_default(col: &AColumn) -> Result<SqlVal> {
    if let Some(val) = col.default() {
        return Ok(val.clone());
    }
    if col.nullable() {
        return Ok(SqlVal::Null);
    }
    Ok(match col.typeid()? {
        TypeIdentifier::Ty(ty) => match ty {
            SqlType::Bool => SqlVal::Bool(false),
            SqlType::Int => SqlVal::Int(0),
            SqlType::BigInt => SqlVal::Int(0),
            SqlType::Real => SqlVal::Real(0.0),
            SqlType::Text => SqlVal::Text("".to_string()),
            SqlType::Blob => SqlVal::Blob(Vec::new()),
            #[cfg(feature = "json")]
            SqlType::Json => SqlVal::Json(serde_json::Value::default()),
            #[cfg(feature = "datetime")]
            SqlType::Timestamp => {
                SqlVal::Timestamp(NaiveDateTime::from_timestamp_opt(0, 0).unwrap())
            }
            SqlType::Custom(_) => return Err(Error::NoCustomDefault),
        },
        TypeIdentifier::Name(_) => return Err(Error::NoCustomDefault),
    })
}

/// Writes to `w` the SQL of the list of `columns`.
pub fn list_columns(columns: &[Column], w: &mut impl Write) {
    let mut colnames: Vec<&'static str> = Vec::new();
    columns.iter().for_each(|c| colnames.push(c.name()));
    write!(
        w,
        "{}",
        colnames
            .iter()
            .map(|x| quote_reserved_word(x))
            .collect::<Vec<Cow<str>>>()
            .join(", ")
    )
    .unwrap();
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
                write!(w, "INNER JOIN {} ON ", quote_reserved_word(join_table)).unwrap();
                sql_column(col1, w);
                w.write_str(" = ").unwrap();
                sql_column(col2, w);
            }
        }
    }
}

fn sql_column(col: query::Column, w: &mut impl Write) {
    match col.table() {
        Some(table) => write!(
            w,
            "{}.{}",
            quote_reserved_word(table),
            quote_reserved_word(col.name())
        ),
        None => w.write_str(col.name()),
    }
    .unwrap()
}

pub fn sql_literal_value(val: SqlVal) -> Result<String> {
    use SqlVal::*;
    match val {
        SqlVal::Null => Ok("NULL".to_string()),
        SqlVal::Bool(val) => Ok(val.to_string()),
        Int(val) => Ok(val.to_string()),
        BigInt(val) => Ok(val.to_string()),
        Real(val) => Ok(val.to_string()),
        Text(val) => Ok(format!("'{val}'")),
        Blob(val) => Ok(format!("x'{}'", hex::encode_upper(val))),
        #[cfg(feature = "json")]
        Json(val) => Ok(format!("{val}")),
        #[cfg(feature = "datetime")]
        Timestamp(ndt) => Ok(ndt.format("'%Y-%m-%dT%H:%M:%S%.f'").to_string()),
        Custom(val) => Err(Error::LiteralForCustomUnsupported((*val).clone())),
    }
}
