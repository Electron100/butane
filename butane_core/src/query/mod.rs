//! Types to support database queries. Most users will use
//! the `query!`, `filter!`, and `find!` macros instead of using this
//! module directly.

use crate::db::{BackendRows, ConnectionMethods, QueryResult};
use crate::{DataResult, Result, SqlVal};
use fallible_iterator::FallibleIterator;
use std::marker::PhantomData;

mod fieldexpr;

pub use fieldexpr::{DataOrd, FieldExpr, ManyFieldExpr};

/// Abstract representation of a database expression.
#[derive(Clone)]
pub enum Expr {
    /// A column, referenced by name.
    Column(&'static str),
    /// A value.
    Val(SqlVal),
    /// A placeholder for a value.
    Placeholder,
    /// A boolean condition.
    Condition(Box<BoolExpr>),
}

/// Abstract representation of a boolean expression.
#[derive(Clone)]
pub enum BoolExpr {
    True,
    Eq(&'static str, Expr),
    Ne(&'static str, Expr),
    Lt(&'static str, Expr),
    Gt(&'static str, Expr),
    Le(&'static str, Expr),
    Ge(&'static str, Expr),
    Like(&'static str, Expr),
    AllOf(Vec<BoolExpr>),
    And(Box<BoolExpr>, Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),
    /// Expression which is true if the value of `col` is present in
    /// the set of values of `tbl2_col` where `expr` evaluated on a row
    /// in `tbl2` is true.
    Subquery {
        col: &'static str,
        tbl2: &'static str,
        tbl2_col: &'static str,
        expr: Box<BoolExpr>,
    },
    In(&'static str, Vec<SqlVal>),
    /// Expression which is true if the value of `col` is present in
    /// the set of values of `col2` where `expr` evaluated on a row
    /// in `tbl2` with the specified joins is true.
    SubqueryJoin {
        col: &'static str,
        tbl2: &'static str,
        col2: Column,
        joins: Vec<Join>,
        expr: Box<BoolExpr>,
    },
}

/// Represents the direction of a sort.
#[derive(Clone)]
pub enum OrderDirection {
    Ascending,
    Descending,
}

/// Represents a sorting term (ORDER BY in SQL).
#[derive(Clone)]
pub struct Order {
    pub direction: OrderDirection,
    pub column: &'static str,
}

#[derive(Clone)]
pub enum Join {
    /// Inner join `join_table` where `col1` is equal to
    /// `col2`
    Inner {
        join_table: &'static str,
        col1: Column,
        col2: Column,
    },
}

#[derive(Clone)]
pub struct Column {
    table: Option<&'static str>,
    name: &'static str,
}
impl Column {
    pub fn new(table: &'static str, name: &'static str) -> Self {
        Column {
            table: Some(table),
            name,
        }
    }
    pub fn unqualified(name: &'static str) -> Self {
        Column { table: None, name }
    }
    pub fn table(&self) -> Option<&'static str> {
        self.table
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// Representation of a database query.
#[derive(Clone)]
pub struct Query<T: DataResult> {
    table: &'static str,
    filter: Option<BoolExpr>,
    limit: Option<i32>,
    sort: Vec<Order>,
    phantom: PhantomData<T>,
}
impl<T: DataResult> Query<T> {
    /// Creates a query which matches all objects in `table`. The set
    /// of matched objects can be restricted with `filter` and
    /// `limit`.
    pub fn new(table: &'static str) -> Query<T> {
        Query {
            table,
            filter: None,
            limit: None,
            sort: Vec::new(),
            phantom: PhantomData,
        }
    }

    /// Restricts the query to matching only objects for which `expr`
    /// is true. Returns `self` as this method is expected to be
    /// chained.
    pub fn filter(mut self, expr: BoolExpr) -> Query<T> {
        self.filter = Some(expr);
        self
    }

    /// Limits the query to matching the first `lim` objects. Returns
    /// `self` as this method is expected to be chained.
    pub fn limit(mut self, lim: i32) -> Query<T> {
        self.limit = Some(lim);
        self
    }

    /// Order the query results by the given column. Multiple calls to
    /// this method may be made, with earlier calls taking precedence.
    /// It is recommended to use the `colname!`
    /// macro to construct the column name in a typesafe manner.
    pub fn order(mut self, column: &'static str, direction: OrderDirection) -> Query<T> {
        self.sort.push(Order { direction, column });
        self
    }

    /// Shorthand for `order(column, OrderDirection::Ascending)`
    pub fn order_asc(self, column: &'static str) -> Query<T> {
        self.order(column, OrderDirection::Ascending)
    }

    /// Shorthand for `order(column, OrderDirection::Descending)`
    pub fn order_desc(self, column: &'static str) -> Query<T> {
        self.order(column, OrderDirection::Descending)
    }

    /// Executes the query against `conn` and returns the first result (if any).
    pub fn load_first(self, conn: &impl ConnectionMethods) -> Result<Option<T>> {
        conn.query(self.table, T::COLUMNS, self.filter, Some(1), None)?
            .mapped(T::from_row)
            .nth(0)
    }

    /// Executes the query against `conn`.
    pub fn load(self, conn: &impl ConnectionMethods) -> Result<QueryResult<T>> {
        let sort = if self.sort.is_empty() {
            None
        } else {
            Some(self.sort.as_slice())
        };
        conn.query(self.table, T::COLUMNS, self.filter, self.limit, sort)?
            .mapped(T::from_row)
            .collect()
    }

    /// Executes the query against `conn` and deletes all matching objects.
    pub fn delete(self, conn: &impl ConnectionMethods) -> Result<usize> {
        conn.delete_where(self.table, self.filter.unwrap_or(BoolExpr::True))
    }
}
