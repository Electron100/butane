use crate::db::{BackendConnection, QueryResult};
use crate::{DBResult, Result, SqlVal};
use std::marker::PhantomData;

#[derive(Clone)]
pub enum Expr {
    Column(&'static str),
    Val(SqlVal),
    Placeholder,
    Condition(Box<BoolExpr>),
}

#[derive(Clone)]
pub enum BoolExpr {
    Eq(&'static str, Expr),
    Ne(&'static str, Expr),
    Lt(&'static str, Expr),
    Gt(&'static str, Expr),
    Le(&'static str, Expr),
    Ge(&'static str, Expr),
    And(Box<BoolExpr>, Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),
    // col, tbl2, tbl2_col
    Subquery(&'static str, &'static str, &'static str, Box<BoolExpr>),
}

pub trait AsExpr {
    fn as_expr(self) -> Expr;
}

impl AsExpr for Expr {
    fn as_expr(self) -> Expr {
        self
    }
}

impl<T> AsExpr for T
where
    T: Into<SqlVal>,
{
    fn as_expr(self) -> Expr {
        Expr::Val(self.into())
    }
}

#[derive(Clone)]
pub struct Query<T: DBResult> {
    table: &'static str,
    filter: Option<BoolExpr>,
    limit: Option<i32>,
    phantom: PhantomData<T>,
}
impl<T: DBResult> Query<T> {
    pub fn new(table: &'static str) -> Query<T> {
        Query {
            table,
            filter: None,
            limit: None,
            phantom: PhantomData,
        }
    }
    pub fn filter(mut self, expr: BoolExpr) -> Query<T> {
        self.filter = Some(expr);
        self
    }
    pub fn limit(mut self, lim: i32) -> Query<T> {
        self.limit = Some(lim);
        self
    }

    pub fn load(self, conn: &impl BackendConnection) -> Result<QueryResult<T>> {
        conn.query(self.table, T::COLUMNS, self.filter, self.limit)?
            .into_iter()
            .map(|row| T::from_row(row))
            .collect()
    }
}
