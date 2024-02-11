use std::fmt::Debug;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Filesystem abstraction for `Migrations`. Primarily intended to
/// allow bypassing the real filesystem during testing, but
/// implementations that do not call through to the real filesystem
/// are supported in production.
pub trait Filesystem: Debug {
    /// Ensure a directory exists, recursively creating missing components
    fn ensure_dir(&self, path: &Path) -> std::io::Result<()>;
    /// List all paths in a directory
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>>;
    /// Opens a file for writing. Creates it if it does not exist. Truncates it otherwise.
    fn write(&self, path: &Path) -> std::io::Result<Box<dyn Write>>;
    /// Opens a file for reading.
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>>;
}

/// `[Filesystem`] implementation using [`std::fs`].
#[derive(Debug)]
pub struct OsFilesystem;

impl Filesystem for OsFilesystem {
    fn ensure_dir(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>> {
        std::fs::read_dir(path)?
            .map(|entry| entry.map(|de| de.path()))
            .collect()
    }
    fn write(&self, path: &Path) -> std::io::Result<Box<dyn Write>> {
        std::fs::File::create(path).map(|f| Box::new(f) as Box<dyn Write>)
    }
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>> {
        std::fs::File::open(path).map(|f| Box::new(f) as Box<dyn Read>)
    }
}
