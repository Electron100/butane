//! Not expected to be used directly.

use std::borrow::{Borrow, Cow};
use std::marker::PhantomData;

use crate::fkey::ForeignKey;
use crate::query::{BoolExpr, Column, Expr, Join};
use crate::sqlval::{FieldType, SqlVal, ToSql};
use crate::DataObject;

macro_rules! binary_op {
    ($func_name:ident, $bound:path, $cond:ident) => {
        /// Creates a [BoolExpr] which evaluates this column against `val`.
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
/// Unlike `PartialOrd`, handles `Option`, which we need for nullable types.
pub trait DataOrd<Rhs> {}
impl<T> DataOrd<T> for Option<T> where T: PartialOrd<T> + FieldType {}
impl<T> DataOrd<T> for T where T: PartialOrd<T> + FieldType {}

/// Used to implement the `query!` and `filter!` macros.
/// Manual use of this type is not recommended, but not prohibited.
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
    /// Creates a `FieldExpr` from its name.
    pub fn new(name: &'static str) -> Self {
        FieldExpr {
            name,
            phantom: PhantomData,
        }
    }

    /// Returns the name of this field.
    pub fn name(&self) -> &'static str {
        self.name
    }

    binary_op!(eq, std::cmp::PartialEq<U>, Eq);
    binary_op!(ne, std::cmp::PartialEq<U>, Ne);
    binary_op!(lt, DataOrd<U>, Lt);
    binary_op!(gt, DataOrd<U>, Gt);
    binary_op!(le, DataOrd<U>, Le);
    binary_op!(ge, DataOrd<U>, Ge);

    /// Creates a [BoolExpr] which will evaluate to true if
    /// the value of this field is "like" `val`, where
    /// "like" is evaluated as the SQL LIKE operator.
    pub fn like<U>(&self, val: U) -> BoolExpr
    where
        U: ToSql,
    {
        BoolExpr::Like(self.name, Expr::Val(val.to_sql()))
    }

    /// Creates a [BoolExpr] which will evaluate to true if
    /// the value of this field is contained in `vals`.
    pub fn is_in<U: ToSql>(&self, vals: Vec<U>) -> BoolExpr {
        BoolExpr::In(self.name, vals.into_iter().map(|v| v.to_sql()).collect())
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
