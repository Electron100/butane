use propane::db::{Backend, Connection};
use propane::migrations::Migration;
use rsfs;
use rsfs::{DirEntry, GenFS};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub mod blog;

struct MemoryFilesystem {
    fs: rsfs::mem::FS,
}
impl MemoryFilesystem {
    fn new() -> Self {
        MemoryFilesystem {
            fs: rsfs::mem::FS::new(),
        }
    }
}
impl propane::migrations::Filesystem for MemoryFilesystem {
    fn ensure_dir(&self, path: &Path) -> std::io::Result<()> {
        self.fs.create_dir_all(path)
    }
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>> {
        self.fs
            .read_dir(path)?
            .map(|entry| entry.map(|de| de.path()))
            .collect()
    }
    fn write(&self, path: &Path) -> std::io::Result<Box<dyn Write>> {
        self.fs.create_file(path).map(|f| Box::new(f) as Box<Write>)
    }
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>> {
        self.fs.open_file(path).map(|f| Box::new(f) as Box<Read>)
    }
}

pub fn setup_db(backend: Box<Backend>, conn: &Connection) {
    let mut root = std::env::current_dir().unwrap();
    root.push("propane/migrations");
    let disk_migrations = propane::migrations::from_root(&root);
    let current = disk_migrations.get_current();
    let mem_migrations =
        propane::migrations::from_root_and_filesystem("/", MemoryFilesystem::new());
    let initial: Migration = mem_migrations
        .create_migration_sql(&backend, &format!("init"), None, &current)
        .expect("expected to create migration without error")
        .expect("expected non-None migration");
    let sql = initial.get_up_sql(backend.get_name()).unwrap();
    conn.execute(&sql).unwrap();
}

#[macro_export]
macro_rules! maketest {
    ($fname:ident, $backend:expr, $connstr:expr) => {
        paste::item! {
            #[test]
            pub fn [<$fname _ $backend>]() {
                let backend = propane::db::get_backend(&stringify!($backend)).unwrap();
                let conn = backend.connect(&$connstr).unwrap();
                crate::common::setup_db(backend, &conn);
                $fname(conn);
            }
        }
    };
}

#[macro_export]
macro_rules! testall {
    ($fname:ident) => {
        maketest!($fname, sqlite, format!(":memory:"));
    };
}
