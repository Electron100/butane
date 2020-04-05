use rsfs;
use rsfs::{DirEntry, GenFS};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct MemoryFilesystem {
    fs: rsfs::mem::FS,
}
impl MemoryFilesystem {
    pub fn new() -> Self {
        MemoryFilesystem {
            fs: rsfs::mem::FS::new(),
        }
    }
}
impl super::fs::Filesystem for MemoryFilesystem {
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
        self.fs
            .create_file(path)
            .map(|f| Box::new(f) as Box<dyn Write>)
    }
    fn read(&self, path: &Path) -> std::io::Result<Box<dyn Read>> {
        self.fs
            .open_file(path)
            .map(|f| Box::new(f) as Box<dyn Read>)
    }
}
