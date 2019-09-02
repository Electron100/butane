//! Not expected to be used directly.

use crate::fkey::ForeignKey;
use crate::query::{BoolExpr, Column, Expr, Join};
use crate::sqlval::{FieldType, IntoSql, SqlVal, ToSql};
use crate::DataObject;
use std::borrow::Borrow;
use std::marker::PhantomData;

macro_rules! binary_op {
    ($func_name:ident, $bound:path, $cond:ident) => {
        pub fn $func_name<U>(&self, val: U) -> BoolExpr
        where
            U: $bound + ToSql,
        {
            BoolExpr::$cond(self.name, Expr::Val(val.to_sql()))
        }
    };
}

/// Used to implement the `query!` and `filter!` macros.
pub struct FieldExpr<T>
where
    T: Into<SqlVal>,
{
    name: &'static str,
    phantom: PhantomData<T>,
}

impl<T> FieldExpr<T>
where
    T: Into<SqlVal>,
{
    pub fn new(name: &'static str) -> Self {
        FieldExpr {
            name,
            phantom: PhantomData,
        }
    }

    binary_op!(eq, std::cmp::PartialEq<T>, Eq);
    binary_op!(ne, std::cmp::PartialEq<T>, Ne);
    binary_op!(lt, std::cmp::PartialOrd<T>, Lt);
    binary_op!(gt, std::cmp::PartialOrd<T>, Gt);
    binary_op!(le, std::cmp::PartialOrd<T>, Le);
    binary_op!(ge, std::cmp::PartialOrd<T>, Ge);
}
impl<F: DataObject> FieldExpr<ForeignKey<F>> {
    pub fn subfilter(&self, q: BoolExpr) -> BoolExpr {
        BoolExpr::Subquery {
            col: self.name,
            tbl2: F::TABLE,
            tbl2_col: F::PKCOL,
            expr: Box::new(q),
        }
    }
    pub fn subfilterpk(&self, pk: F::PKType) -> BoolExpr {
        self.subfilter(BoolExpr::Eq(
            F::PKCOL,
            crate::query::Expr::Val(pk.into_sql()),
        ))
    }
    pub fn fields(&self) -> F::Fields {
        F::Fields::default()
    }
}

pub struct ManyFieldExpr<O, T>
where
    O: DataObject, // owner
    T: DataObject, // owned
{
    many_table: &'static str,
    phantomo: PhantomData<O>,
    phantomt: PhantomData<T>,
}
impl<O, T> ManyFieldExpr<O, T>
where
    O: DataObject,
    T: DataObject,
{
    pub fn new(many_table: &'static str) -> Self {
        ManyFieldExpr {
            many_table,
            phantomo: PhantomData,
            phantomt: PhantomData,
        }
    }
    pub fn contains(&self, q: BoolExpr) -> BoolExpr {
        //let many_tbl = format!("{}_{}_Many", O::TABLE, self.name);
        BoolExpr::SubqueryJoin {
            col: O::PKCOL,
            tbl2: T::TABLE,
            col2: Column::new(self.many_table, "owner"),
            joins: vec![Join::Inner {
                join_table: self.many_table,
                col1: Column::new(self.many_table, "has"),
                col2: Column::new(T::TABLE, T::PKCOL),
            }],
            expr: Box::new(q),
        }
    }
    pub fn containspk(&self, pk: impl Borrow<<T::PKType as FieldType>::RefType>) -> BoolExpr {
        self.contains(BoolExpr::Eq(
            T::PKCOL,
            crate::query::Expr::Val(pk.borrow().to_sql()),
        ))
    }
    pub fn fields(&self) -> T::Fields {
        T::Fields::default()
    }
}
