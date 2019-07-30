use proc_macro_hack::proc_macro_hack;
pub use propane_codegen::model;
pub use propane_core::db;
pub use propane_core::fkey::ForeignKey;
pub use propane_core::migrations;
pub use propane_core::query;
pub use propane_core::{
    DBObject, DBResult, Error, FieldType, FromSql, IntoSql, Result, SqlType, SqlVal, ToSql,
};

#[proc_macro_hack]
pub use propane_codegen::filter;

pub mod prelude {
    pub use filter;
    pub use propane_codegen::Model;
    pub use propane_core::DBObject;
}

#[macro_export]
macro_rules! query {
    ($model:ident, $filter:expr) => {
        $model::query().filter(filter!($model, $filter))
    };
}

#[macro_export]
macro_rules! find {
    ($dbobj:ident, $filter:expr, $conn:expr) => {
        propane::query!($dbobj, $filter)
            .limit(1)
            .load($conn)
            .and_then(|mut results| results.pop().ok_or(propane::Error::NoSuchObject))
    };
}
