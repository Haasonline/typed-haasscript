//! Where `ths strip` writes each stripped file: beside its source, or below
//! the output directory at its path relative to the root, as `tsc` does
//! with `outDir` and `rootDir`.

use std::path::{Path, PathBuf};

use crate::Project;
use crate::file_extensions::UPLOAD;

impl Project {
    /// The directory stripped files go to, when the settings name one.
    pub fn output_directory(&self) -> Option<PathBuf> {
        self.settings.output.as_ref().map(|output| self.directory.join(output))
    }

    /// The stripped path of each of `sources` (absolute paths), in order.
    pub fn stripped_paths(&self, sources: &[PathBuf]) -> Vec<PathBuf> {
        let Some(output) = self.output_directory() else {
            return sources.iter().map(|source| stripped_beside(source)).collect();
        };
        let root = match &self.settings.root {
            Some(root) => self.directory.join(root),
            None => common_directory(sources).unwrap_or_else(|| self.directory.clone()),
        };
        sources
            .iter()
            .map(|source| match source.strip_prefix(&root) {
                Ok(relative) => output.join(relative).with_extension(UPLOAD),
                Err(_) => stripped_beside(source),
            })
            .collect()
    }
}

/// `name.hs` beside `name.ths`.
pub fn stripped_beside(source: &Path) -> PathBuf {
    source.with_extension(UPLOAD)
}

/// The deepest directory holding every one of `files`.
fn common_directory(files: &[PathBuf]) -> Option<PathBuf> {
    let mut common = files.first()?.parent()?.to_path_buf();
    for file in &files[1..] {
        while !file.starts_with(&common) {
            common = common.parent()?.to_path_buf();
        }
    }
    Some(common)
}
