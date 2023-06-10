//! Not expected to be used directly.

use crate::fkey::ForeignKey;
use crate::query::{BoolExpr, Column, Expr, Join};
use crate::sqlval::{FieldType, SqlVal, ToSql};
use crate::DataObject;
use std::borrow::{Borrow, Cow};
use std::cmp::{PartialEq, PartialOrd};
use std::marker::PhantomData;

macro_rules! binary_op {
    ($func_name:ident, $bound:path, $cond:ident) => {
        pub fn $func_name<U>(&self, val: &U) -> BoolExpr
        where
            T: $bound,
            U: ToSql,
        {
            BoolExpr::$cond(self.name, Expr::Val(val.to_sql()))
        }
    };
}

/// Marker trait to determine whether values can be compared.
///
/// Unlike `PartialEq`, handles `Option`, which we need for nullable
/// types. We would like to automatically implement it if PartialEq
/// is implemented, but we can't do that without specialization or
/// negative trait bounds.
pub trait DataEq<Rhs> {}
impl<T> DataEq<T> for Option<T> where T: PartialEq<T> + FieldType {}
impl<T> DataEq<T> for T where T: PartialEq<T> + FieldType {}

/// Marker trait to determine whether values can be compared.
/// Unlike `PartialOrd`, handles `Option`, which we need for nullable types.
pub trait DataOrd<Rhs> {}
impl<T> DataOrd<T> for Option<T> where T: PartialOrd<T> + FieldType {}
impl<T> DataOrd<T> for T where T: PartialOrd<T> + FieldType {}

/// Used to implement the `query!` and `filter!` macros.
#[derive(Clone, Debug)]
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

    pub fn name(&self) -> &'static str {
        self.name
    }

    binary_op!(eq, std::cmp::PartialEq<U>, Eq);
    binary_op!(ne, std::cmp::PartialEq<U>, Ne);
    binary_op!(lt, DataOrd<U>, Lt);
    binary_op!(gt, DataOrd<U>, Gt);
    binary_op!(le, DataOrd<U>, Le);
    binary_op!(ge, DataOrd<U>, Ge);

    pub fn like<U>(&self, val: U) -> BoolExpr
    where
        U: ToSql,
    {
        BoolExpr::Like(self.name, Expr::Val(val.to_sql()))
    }
}
impl<F: DataObject> FieldExpr<ForeignKey<F>> {
    pub fn subfilter(&self, q: BoolExpr) -> BoolExpr {
        BoolExpr::Subquery {
            col: self.name,
            tbl2: Cow::Borrowed(F::TABLE),
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

#[derive(Clone, Debug)]
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
            tbl2: Cow::Borrowed(T::TABLE),
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
