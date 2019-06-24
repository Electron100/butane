pub use propane_codegen::model;
pub use propane_core::adb;
pub use propane_core::db;
pub use propane_core::field;
pub use propane_core::migrations;
pub use propane_core::query;
pub use propane_core::{DBObject, Error, Result, SqlType};

pub mod prelude {
    pub use propane_core::sqlval::SqlInto;
    pub use propane_core::DBObject;
}

macro_rules! query {
    ($dbojb:ty, $filter:expr) => {
        $dbobj::query().filter(filter!($filter))
    };
}
