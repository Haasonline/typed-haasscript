//! Where checking reads a file's text from: the disk for `ths check`, and
//! for the language server the editor's open documents, unsaved changes
//! included, before the disk.

use std::path::Path;

pub trait SourceFiles {
    fn read(&self, file: &Path) -> std::io::Result<String>;
}

/// Every file as it is saved.
pub struct Disk;

impl SourceFiles for Disk {
    fn read(&self, file: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(file)
    }
}
