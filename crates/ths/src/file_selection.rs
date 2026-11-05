//! Which project and files a command works on. Without paths, the files
//! the project's `ths.toml` includes, as `tsc` reads `tsconfig.json`. With
//! paths, those files, and the source files below those directories; the
//! project's settings still apply to them.

use std::path::{Path, PathBuf};

use ths_project::{Project, source_files_under};

pub struct Selection {
    pub project: Option<Project>,
    pub files: Vec<PathBuf>,
}

/// The project at `project_path`, or the nearest one above the current
/// directory, and the files to work on. A file found by listing a
/// directory or the project is kept only when `wanted` accepts it; a file
/// named in `paths` is always kept, so the command can say why it cannot
/// take it.
pub fn select(project_path: Option<&Path>, paths: &[PathBuf], wanted: fn(&Path) -> bool) -> Result<Selection, String> {
    let current =
        std::env::current_dir().map_err(|error| format!("ths: cannot read the current directory: {error}"))?;
    let project = match project_path {
        Some(path) => Some(Project::load(&current.join(path))),
        None => Project::find(&current).transpose(),
    }
    .transpose()
    .map_err(|error| error.to_string())?;
    let files = if paths.is_empty() {
        let project = project.as_ref().ok_or_else(|| {
            format!("ths: no {} here or in any directory above, and no files given", ths_project::SETTINGS_FILE)
        })?;
        project.included_files().map_err(|error| error.to_string())?.into_iter().filter(|file| wanted(file)).collect()
    } else {
        let skipped: Vec<PathBuf> = project.iter().filter_map(Project::output_directory).collect();
        let mut files = Vec::new();
        for path in paths {
            let path = current.join(path);
            if path.is_dir() {
                files.extend(source_files_under(&path, &skipped).into_iter().filter(|file| wanted(file)));
            } else {
                files.push(path);
            }
        }
        files
    };
    Ok(Selection { project, files })
}
