//! `ths init`: makes a directory a project by writing a `ths.toml` there,
//! never over one that exists. The editor plugins run it too, so what a
//! new project file holds is decided here alone.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use ths_project::{SETTINGS_FILE, SETTINGS_TEMPLATE};

use crate::shown_paths::shown;

/// Writes the settings file in `directory`; returns its path.
pub fn create_settings_file(directory: &Path) -> Result<PathBuf, String> {
    if !directory.is_dir() {
        return Err(format!("ths init: '{}' is not a directory", shown(directory).display()));
    }
    let settings_file = directory.join(SETTINGS_FILE);
    let shown_file = shown(&settings_file);
    // Created only if absent, so a file made meanwhile is never replaced.
    let mut file = OpenOptions::new().write(true).create_new(true).open(&settings_file).map_err(|error| {
        if error.kind() == ErrorKind::AlreadyExists {
            format!("ths init: '{}' already exists", shown_file.display())
        } else {
            format!("ths init: cannot write '{}': {error}", shown_file.display())
        }
    })?;
    file.write_all(SETTINGS_TEMPLATE.as_bytes())
        .map_err(|error| format!("ths init: cannot write '{}': {error}", shown_file.display()))?;
    Ok(settings_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_file_is_written_from_the_template() {
        let directory = tempfile::tempdir().unwrap();
        let written = create_settings_file(directory.path()).unwrap();
        assert_eq!(written, directory.path().join(SETTINGS_FILE));
        assert_eq!(std::fs::read_to_string(written).unwrap(), SETTINGS_TEMPLATE);
    }

    #[test]
    fn an_existing_settings_file_is_kept() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join(SETTINGS_FILE), "[check]\nstrict = true\n").unwrap();
        let error = create_settings_file(directory.path()).unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        let kept = std::fs::read_to_string(directory.path().join(SETTINGS_FILE)).unwrap();
        assert_eq!(kept, "[check]\nstrict = true\n");
    }

    #[test]
    fn a_missing_directory_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let error = create_settings_file(&directory.path().join("missing")).unwrap_err();
        assert!(error.contains("is not a directory"), "{error}");
    }
}
