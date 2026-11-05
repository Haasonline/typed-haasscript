//! The declarations every checked file sees: the built-in ones, the
//! project's own `.d.ths` files, and the `CC_` declaration of each custom
//! command a project file defines, so a script can call a command written
//! in another file, typed or plain, as on the server, where every saved
//! command is available by name. A file sees every one but its own, so a
//! declaration file does not clash with itself.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ths_check::{Declarations, command_declaration};
use ths_project::{Project, file_extensions};

use crate::shown_paths::shown;
use crate::source_dialect::dialect_of;
use crate::source_files::SourceFiles;

pub struct ProjectDeclarations {
    built_in: Declarations,
    /// What each project file declares: a declaration file itself, or the
    /// `CC_` declaration of the command a script defines.
    contributions: Vec<(PathBuf, Declarations)>,
    /// Commands defined in more than one file; the first file's is used.
    pub duplicates: Vec<String>,
}

impl ProjectDeclarations {
    /// The declarations `file` sees: every project file's but its own.
    pub fn for_file(&self, file: &Path) -> Declarations {
        combined(&self.built_in, self.contributions.iter().filter(|(contributor, _)| contributor != file))
    }
}

/// A command a script defines that an earlier file already defined.
struct DuplicateCommand<'files> {
    file: &'files Path,
    command: String,
    first: &'files Path,
}

/// The commands the scripts define, as declarations, and those defined
/// again.
struct CommandDefinitions<'files> {
    contributions: Vec<(PathBuf, Declarations)>,
    duplicates: Vec<DuplicateCommand<'files>>,
}

/// The declarations for checking `files`: the project's included files
/// and `files` themselves are read for declaration files and commands.
pub fn load(
    project: Option<&Project>,
    files: &[PathBuf],
    reader: &dyn SourceFiles,
) -> Result<ProjectDeclarations, String> {
    let sources = contributing_files(project, files)?;
    let read: Vec<(&PathBuf, String)> =
        sources.iter().filter_map(|path| Some((path, reader.read(path).ok()?))).collect();
    let (declaration_files, scripts): (Vec<_>, Vec<_>) = read.iter().partition(|(path, _)| is_declaration_file(path));
    let built_in = Declarations::built_in();
    let mut contributions: Vec<(PathBuf, Declarations)> =
        declaration_files.iter().map(|(path, text)| ((*path).clone(), Declarations::from_sources(&[text]))).collect();
    // A command's interface types are checked against the declaration
    // files, without the other commands.
    let base = combined(&built_in, &contributions);
    let commands = defined_commands(&scripts, &base);
    let duplicates = commands.duplicates.iter().map(duplicate_message).collect();
    contributions.extend(commands.contributions);
    Ok(ProjectDeclarations { built_in, contributions, duplicates })
}

/// The files read for declarations: the project's included files, then
/// any of `files` it does not include.
fn contributing_files(project: Option<&Project>, files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut sources: Vec<PathBuf> = match project {
        Some(project) => project.included_files().map_err(|error| error.to_string())?,
        None => Vec::new(),
    };
    for file in files {
        if !sources.contains(file) {
            sources.push(file.clone());
        }
    }
    Ok(sources)
}

/// `built_in` with every one of `contributions` added.
fn combined<'contributions>(
    built_in: &Declarations,
    contributions: impl IntoIterator<Item = &'contributions (PathBuf, Declarations)>,
) -> Declarations {
    contributions
        .into_iter()
        .fold(built_in.clone(), |declarations, (_, contributed)| declarations.with_declarations(contributed))
}

/// The `CC_` declaration of each command `scripts` define, checked
/// against `base`; when two files define a command, the first one's is
/// used.
fn defined_commands<'files>(
    scripts: &[&'files (&'files PathBuf, String)],
    base: &Declarations,
) -> CommandDefinitions<'files> {
    let mut defined_in: HashMap<String, &PathBuf> = HashMap::new();
    let mut definitions = CommandDefinitions { contributions: Vec::new(), duplicates: Vec::new() };
    for (source_path, source) in scripts {
        let Some(dialect) = dialect_of(source_path) else { continue };
        let Some(declaration) = command_declaration(source, dialect, base) else { continue };
        match defined_in.get(&declaration.command) {
            Some(first) => {
                definitions.duplicates.push(DuplicateCommand { file: source_path, command: declaration.command, first })
            }
            None => {
                defined_in.insert(declaration.command, source_path);
                definitions
                    .contributions
                    .push(((*source_path).clone(), Declarations::from_sources(&[&declaration.text])));
            }
        }
    }
    definitions
}

/// What is told about a command defined twice.
fn duplicate_message(duplicate: &DuplicateCommand) -> String {
    format!(
        "ths: {}: {} is also defined in {}; using that one",
        shown(duplicate.file).display(),
        duplicate.command,
        shown(duplicate.first).display()
    )
}

fn is_declaration_file(path: &Path) -> bool {
    path.to_string_lossy().ends_with(file_extensions::DECLARATION)
}

#[cfg(test)]
mod tests {
    use ths_check::{CheckOptions, check_with};
    use ths_syntax::Dialect;

    use super::*;

    const DECLARATION_FILE: &str = "interface PairResult { label: string }\ndeclare function Pair(): PairResult\n";

    fn project() -> ProjectDeclarations {
        ProjectDeclarations {
            built_in: Declarations::built_in(),
            contributions: vec![(PathBuf::from("pair.d.ths"), Declarations::from_sources(&[DECLARATION_FILE]))],
            duplicates: Vec::new(),
        }
    }

    #[test]
    fn a_declaration_file_does_not_clash_with_itself() {
        let declarations = project().for_file(Path::new("pair.d.ths"));
        assert_eq!(check_with(DECLARATION_FILE, Dialect::Typed, &declarations, CheckOptions::default()), []);
    }

    #[test]
    fn another_file_sees_the_declaration_file() {
        let declarations = project().for_file(Path::new("bot.ths"));
        let diagnostics =
            check_with("local label: number = Pair().label", Dialect::Typed, &declarations, CheckOptions::default());
        let messages: Vec<String> = diagnostics.into_iter().map(|diagnostic| diagnostic.message).collect();
        assert_eq!(messages, ["Type 'string' is not assignable to type 'number'."]);
    }
}
