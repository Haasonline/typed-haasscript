//! Which source files a project, or a directory, holds. As in TypeScript,
//! a pattern without wildcards that names a directory means everything
//! below it, wildcards skip names starting with a dot, and the output
//! directory is always left out.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::file_extensions::{PLAIN_LUA, PLAIN_UPLOAD, TYPED_SOURCE, UPLOAD};
use crate::{Project, ProjectError};

const WILDCARDS: [char; 3] = ['*', '?', '['];

/// A `.ths`, `.d.ths`, `.lua` or `.hs` file.
pub fn is_source_file(path: &Path) -> bool {
    let name = path.to_string_lossy();
    [TYPED_SOURCE, PLAIN_LUA, PLAIN_UPLOAD].iter().any(|extension| name.ends_with(extension))
}

/// Every source file below `directory`, in path order, leaving out the
/// directories in `skipped`.
pub fn source_files_under(directory: &Path, skipped: &[PathBuf]) -> Vec<PathBuf> {
    walk_sources(directory, skipped, |_| true)
}

impl Project {
    /// Every source file `include` names and `exclude` does not, in path
    /// order.
    pub fn included_files(&self) -> Result<Vec<PathBuf>, ProjectError> {
        let include = self.patterns(&self.settings.include)?;
        let exclude = self.patterns(&self.settings.exclude)?;
        let skipped: Vec<PathBuf> = self.output_directory().into_iter().collect();
        Ok(walk_sources(&self.directory, &skipped, |relative| {
            include.is_match(relative) && !exclude.is_match(relative)
        }))
    }

    fn patterns(&self, patterns: &[String]) -> Result<GlobSet, ProjectError> {
        let mut set = GlobSetBuilder::new();
        for pattern in patterns {
            let trimmed = pattern.trim_end_matches('/');
            set.add(glob(trimmed)?);
            if !trimmed.contains(WILDCARDS) && self.directory.join(trimmed).is_dir() {
                set.add(glob(&format!("{trimmed}/**"))?);
            }
        }
        set.build().map_err(|error| ProjectError::Pattern { pattern: patterns.join(", "), message: error.to_string() })
    }
}

/// `*` stays within one directory; `**` crosses them.
fn glob(pattern: &str) -> Result<Glob, ProjectError> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|error| ProjectError::Pattern { pattern: pattern.to_string(), message: error.to_string() })
}

/// Source files below `directory` whose path relative to it `wanted`
/// accepts. Names starting with a dot are skipped, and so is a `.hs` file
/// beside a `.ths` file of the same name, which is that file stripped.
fn walk_sources(directory: &Path, skipped: &[PathBuf], wanted: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(directory)
        .into_iter()
        .filter_entry(|entry| {
            let hidden = entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.');
            !hidden && !skipped.iter().any(|skipped| entry.path() == skipped)
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_source_file(path) && !is_stripped_beside_source(path))
        .filter(|path| path.strip_prefix(directory).is_ok_and(&wanted))
        .collect();
    files.sort();
    files
}

/// A `.hs` file beside a `.ths` file of the same name: that file
/// stripped, which is never checked.
pub fn is_stripped_beside_source(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == UPLOAD)
        && path.with_extension(TYPED_SOURCE.trim_start_matches('.')).is_file()
}
