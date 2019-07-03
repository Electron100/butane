use proc_macro_hack::proc_macro_hack;
pub use propane_codegen::model;
pub use propane_core::adb;
pub use propane_core::db;
pub use propane_core::field;
pub use propane_core::migrations;
pub use propane_core::query;
pub use propane_core::{DBObject, DBResult, Error, Result, SqlType};

#[proc_macro_hack]
pub use propane_codegen::filter;

pub mod prelude {
    pub use filter;
    pub use propane_codegen::Model;
    pub use propane_core::sqlval::SqlInto;
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
    ($model:ident, $conn:expr, $filter:expr) => {
        query!($model, $filter)
            .limit(1)
            .load($conn)
            .map(|v| v.first())
    };
}
