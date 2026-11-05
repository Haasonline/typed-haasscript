//! A Typed HaasScript project, the equivalent of a TypeScript project: a
//! `ths.toml` settings file and the source files it includes.

pub mod file_extensions;
mod module_paths;
mod output_paths;
mod project_error;
mod project_files;
mod settings;
mod settings_template;

pub use module_paths::{ModulePathError, normalized, resolve_import};
pub use output_paths::stripped_beside;
pub use project_error::ProjectError;
pub use project_files::{is_source_file, is_stripped_beside_source, source_files_under};
pub use settings::{CheckSettings, SETTINGS_FILE, Settings};
pub use settings_template::SETTINGS_TEMPLATE;

use std::path::{Path, PathBuf};

/// A `ths.toml` and the directory it is in, which its paths are relative to.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub directory: PathBuf,
    pub settings: Settings,
}

impl Project {
    /// The project whose `ths.toml` is in `start` or the nearest directory
    /// above it, as `tsc` finds `tsconfig.json`.
    pub fn find(start: &Path) -> Result<Option<Project>, ProjectError> {
        for directory in start.ancestors() {
            let settings_file = directory.join(SETTINGS_FILE);
            if settings_file.is_file() {
                return Project::load(&settings_file).map(Some);
            }
        }
        Ok(None)
    }

    /// The project at `path`: a `ths.toml`, or a directory holding one.
    pub fn load(path: &Path) -> Result<Project, ProjectError> {
        let settings_file = if path.is_dir() { path.join(SETTINGS_FILE) } else { path.to_path_buf() };
        let text = std::fs::read_to_string(&settings_file)
            .map_err(|error| ProjectError::Read { path: settings_file.clone(), message: error.to_string() })?;
        let settings = Settings::parse(&text)
            .map_err(|message| ProjectError::Settings { path: settings_file.clone(), message })?;
        let directory = settings_file.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Project { directory, settings })
    }
}

#[cfg(test)]
mod tests;
