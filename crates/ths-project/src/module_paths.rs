//! Where an import's path leads (docs/design.md,
//! "Imports"): relative to the importing file, starting with `./` or `../`,
//! without an extension, naming a `.ths` file that is not a declaration
//! file.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::file_extensions::{PLAIN_LUA, PLAIN_UPLOAD, TYPED_SOURCE};

const CURRENT_DIRECTORY: &str = "./";
const PARENT_DIRECTORY: &str = "../";
/// `name.d` names `name.d.ths`, a declaration file.
const DECLARATION_STEM_SUFFIX: &str = ".d";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModulePathError {
    /// Neither `./` nor `../` starts it.
    NotRelative,
    /// It ends with `.ths`, `.lua` or `.hs`.
    HasExtension,
    /// It names a `.d.ths` file, which holds declarations, not code.
    NamesDeclarationFile,
    /// No `.ths` file is there.
    NotFound(PathBuf),
}

impl fmt::Display for ModulePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModulePathError::NotRelative => {
                formatter.write_str("An import path starts with './' or '../': it is relative to the importing file.")
            }
            ModulePathError::HasExtension => {
                formatter.write_str("An import path leaves out the file extension: './averages' names './averages.ths'.")
            }
            ModulePathError::NamesDeclarationFile => {
                formatter.write_str("A declaration file cannot be imported: its declarations are seen by every file of the project already.")
            }
            ModulePathError::NotFound(path) => write!(formatter, "Cannot find the module '{}'.", path.display()),
        }
    }
}

/// The `.ths` file `written` names, imported from `importing_file`.
pub fn resolve_import(importing_file: &Path, written: &str) -> Result<PathBuf, ModulePathError> {
    if !written.starts_with(CURRENT_DIRECTORY) && !written.starts_with(PARENT_DIRECTORY) {
        return Err(ModulePathError::NotRelative);
    }
    if [TYPED_SOURCE, PLAIN_LUA, PLAIN_UPLOAD].iter().any(|extension| written.ends_with(extension)) {
        return Err(ModulePathError::HasExtension);
    }
    if written.ends_with(DECLARATION_STEM_SUFFIX) {
        return Err(ModulePathError::NamesDeclarationFile);
    }
    let directory = importing_file.parent().unwrap_or(Path::new(""));
    let target = normalized(&directory.join(format!("{written}{TYPED_SOURCE}")));
    if target.is_file() { Ok(target) } else { Err(ModulePathError::NotFound(target)) }
}

/// `path` with `.` and `..` folded in, without touching the file system,
/// so the same module reached two ways is one path.
pub fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push(Component::ParentDir);
                }
            }
            other => result.push(other),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn project() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("bots")).unwrap();
        fs::create_dir_all(directory.path().join("math")).unwrap();
        fs::write(directory.path().join("math/averages.ths"), "export local a = 1").unwrap();
        fs::write(directory.path().join("math/averages.lua"), "").unwrap();
        directory
    }

    #[test]
    fn a_path_is_relative_to_the_importing_file() {
        let project = project();
        let bot = project.path().join("bots/trend.ths");
        assert_eq!(resolve_import(&bot, "../math/averages"), Ok(normalized(&project.path().join("math/averages.ths"))));
    }

    #[test]
    fn a_bare_name_is_refused() {
        assert_eq!(resolve_import(Path::new("bot.ths"), "math/averages"), Err(ModulePathError::NotRelative));
    }

    #[test]
    fn an_extension_is_refused() {
        assert_eq!(resolve_import(Path::new("bot.ths"), "./averages.lua"), Err(ModulePathError::HasExtension));
        assert_eq!(resolve_import(Path::new("bot.ths"), "./averages.ths"), Err(ModulePathError::HasExtension));
    }

    #[test]
    fn a_declaration_file_is_refused() {
        assert_eq!(resolve_import(Path::new("bot.ths"), "./commands.d"), Err(ModulePathError::NamesDeclarationFile));
    }

    #[test]
    fn only_a_ths_file_is_found() {
        let project = project();
        fs::remove_file(project.path().join("math/averages.ths")).unwrap();
        let bot = project.path().join("bot.ths");
        assert!(matches!(resolve_import(&bot, "./math/averages"), Err(ModulePathError::NotFound(_))));
    }

    #[test]
    fn dots_are_folded_in() {
        assert_eq!(normalized(Path::new("a/./b/../c.ths")), PathBuf::from("a/c.ths"));
    }
}
