use crate::fkey::ForeignKey;
use crate::query::{BoolExpr, Expr};
use crate::sqlval::{SqlVal, ToSql};
use crate::DBObject;
use std::marker::PhantomData;

macro_rules! binary_op {
    ($func_name:ident, $bound:path, $cond:ident) => {
        pub fn $func_name(&self, val: impl Into<T>) -> BoolExpr
        where
            T: $bound,
        {
            BoolExpr::$cond(self.name, get_val(val))
        }
    };
}

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

    binary_op!(eq, std::cmp::Eq, Eq);
    binary_op!(ne, std::cmp::Eq, Ne);
    binary_op!(lt, std::cmp::Ord, Lt);
    binary_op!(gt, std::cmp::Ord, Gt);
    binary_op!(le, std::cmp::Ord, Le);
    binary_op!(ge, std::cmp::Ord, Ge);
}
impl<F: DBObject> FieldExpr<ForeignKey<F>> {
    pub fn subfilter(&self, q: BoolExpr) -> BoolExpr {
        BoolExpr::Subquery(self.name, F::TABLE, F::PKCOL, Box::new(q))
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

fn get_val<T>(val: impl Into<T>) -> Expr
where
    T: Into<SqlVal>,
{
    let val: T = val.into();
    Expr::Val(val.into())
}
