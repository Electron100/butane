use crate::adb;

mod sqlite;

pub trait Backend {
    fn get_name(&self) -> &'static str;
    fn create_migration_sql(&self, current: &adb::ADB, ops: &[adb::Operation]) -> String;
}

pub fn sqlite_backend() -> impl Backend {
    sqlite::SQLiteBackend::new()
}
