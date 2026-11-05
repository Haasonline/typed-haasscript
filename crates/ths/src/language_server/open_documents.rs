//! The documents the editor has open, with their unsaved text. Checking
//! reads a file from here when it is open and from the disk otherwise, so
//! a file importing an open module sees its unsaved exports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lsp_types::Uri;
use ths_project::normalized;

use crate::source_files::{Disk, SourceFiles};

#[derive(Debug, Clone)]
pub struct OpenDocument {
    /// As the editor names it, for sending its diagnostics back.
    pub uri: Uri,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct OpenDocuments {
    /// By normalized path, in path order so checking is repeatable.
    by_path: BTreeMap<PathBuf, OpenDocument>,
}

impl OpenDocuments {
    pub fn open(&mut self, path: &Path, document: OpenDocument) {
        self.by_path.insert(normalized(path), document);
    }

    /// Replaces the text of an open document; ignored for one not open.
    pub fn change(&mut self, path: &Path, version: i32, text: String) {
        if let Some(document) = self.by_path.get_mut(&normalized(path)) {
            document.version = version;
            document.text = text;
        }
    }

    pub fn close(&mut self, path: &Path) -> Option<OpenDocument> {
        self.by_path.remove(&normalized(path))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &OpenDocument)> {
        self.by_path.iter()
    }
}

impl SourceFiles for OpenDocuments {
    fn read(&self, file: &Path) -> std::io::Result<String> {
        match self.by_path.get(&normalized(file)) {
            Some(document) => Ok(document.text.clone()),
            None => Disk.read(file),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn document(text: &str) -> OpenDocument {
        OpenDocument { uri: Uri::from_str("file:///project/a.ths").unwrap(), version: 1, text: text.to_string() }
    }

    #[test]
    fn an_open_document_is_read_with_its_unsaved_text() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a.ths");
        std::fs::write(&file, "saved").unwrap();
        let mut documents = OpenDocuments::default();
        documents.open(&directory.path().join("./a.ths"), document("unsaved"));
        assert_eq!(documents.read(&file).unwrap(), "unsaved");
        documents.change(&file, 2, "changed".to_string());
        assert_eq!(documents.read(&file).unwrap(), "changed");
        documents.close(&file);
        assert_eq!(documents.read(&file).unwrap(), "saved");
    }
}
