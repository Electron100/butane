use proc_macro_hack::proc_macro_hack;
pub use propane_codegen::model;
pub use propane_core::adb;
pub use propane_core::db;
pub use propane_core::field;
pub use propane_core::migrations;
pub use propane_core::query;
pub use propane_core::{DBObject, Error, Result, SqlType};

#[proc_macro_hack]
pub use propane_codegen::filter;

pub mod prelude {
    pub use filter;
    pub use propane_core::sqlval::SqlInto;
    pub use propane_core::DBObject;
}

#[macro_export]
macro_rules! query {
    ($dbobj:ident, $filter:expr) => {
        $dbobj::query().filter(filter!($dbobj, $filter))
    };
}
