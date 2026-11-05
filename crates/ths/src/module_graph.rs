//! The modules the checked files import, directly or not, and what is
//! wrong with an import that the importing file alone cannot tell: a path
//! that leads nowhere, a file with no exports (only a module can be
//! imported), and circular imports. Each is reported at the import's path.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use ths_check::{Diagnostic, ImportReference, ModuleOutline, Severity, module_outline};
use ths_project::{normalized, resolve_import};

use crate::shown_paths::shown;
use crate::source_dialect::dialect_of;
use crate::source_files::SourceFiles;
use ths_syntax::Dialect;

/// One import that leads to a module.
#[derive(Debug, Clone)]
pub struct ModuleImport {
    pub import: ImportReference,
    pub module: PathBuf,
}

#[derive(Debug, Default)]
pub struct ModuleGraph {
    /// Every import of each file that leads to a module, in source order.
    imports: HashMap<PathBuf, Vec<ModuleImport>>,
    diagnostics: HashMap<PathBuf, Vec<Diagnostic>>,
}

impl ModuleGraph {
    /// Follows the imports of `files` and of every module they reach.
    pub fn build(files: &[PathBuf], sources: &dyn SourceFiles) -> ModuleGraph {
        let mut graph = ModuleGraph::default();
        let mut outlines = Outlines { sources, read: HashMap::new() };
        let mut pending: VecDeque<PathBuf> = files.iter().map(|file| normalized(file)).collect();
        let mut visited = HashSet::new();
        while let Some(file) = pending.pop_front() {
            if !visited.insert(file.clone()) {
                continue;
            }
            let Some(outline) = outlines.of(&file) else { continue };
            for import in outline.imports.into_iter().map(|import| import.reference) {
                match resolve_import(&file, &import.path) {
                    Err(error) => graph.report(&file, &import, error.to_string()),
                    Ok(module) => match outlines.of(&module) {
                        Some(target) if target.is_module => {
                            pending.push_back(module.clone());
                            graph.imports.entry(file.clone()).or_default().push(ModuleImport { import, module });
                        }
                        _ => {
                            let message = format!(
                                "'{}' has no exports, so it cannot be imported: only a module, a file with an export, can be.",
                                import.path
                            );
                            graph.report(&file, &import, message);
                        }
                    },
                }
            }
        }
        graph.report_cycles();
        graph
    }

    /// The imports of `file` that lead to a module.
    pub fn imports_of(&self, file: &Path) -> &[ModuleImport] {
        self.imports.get(&normalized(file)).map(Vec::as_slice).unwrap_or_default()
    }

    /// Every module some file imports.
    pub fn modules(&self) -> impl Iterator<Item = &PathBuf> {
        let mut modules: Vec<&PathBuf> = self.imports.values().flatten().map(|import| &import.module).collect();
        modules.sort();
        modules.dedup();
        modules.into_iter()
    }

    /// What is wrong with the imports of `file`.
    pub fn diagnostics_of(&self, file: &Path) -> &[Diagnostic] {
        self.diagnostics.get(&normalized(file)).map(Vec::as_slice).unwrap_or_default()
    }

    fn report(&mut self, file: &Path, import: &ImportReference, message: String) {
        self.diagnostics.entry(file.to_path_buf()).or_default().push(Diagnostic {
            severity: Severity::Error,
            range: import.range,
            message,
        });
    }

    /// Each import that closes a cycle, at every file of the cycle.
    fn report_cycles(&mut self) {
        let mut found = Vec::new();
        for (file, imports) in &self.imports {
            for edge in imports {
                if let Some(path) = self.path_between(&edge.module, file) {
                    let mut cycle = vec![file.clone()];
                    cycle.extend(path);
                    found.push((file.clone(), edge.import.clone(), cycle));
                }
            }
        }
        for (file, import, cycle) in found {
            let names: Vec<String> = cycle.iter().map(|step| shown(step).display().to_string()).collect();
            self.report(&file, &import, format!("Circular import: {}.", names.join(" -> ")));
        }
    }

    /// The files from `start` to `end` along imports, both included.
    fn path_between(&self, start: &Path, end: &Path) -> Option<Vec<PathBuf>> {
        let mut previous: HashMap<PathBuf, PathBuf> = HashMap::new();
        let mut pending = VecDeque::from([start.to_path_buf()]);
        let mut seen = HashSet::from([start.to_path_buf()]);
        while let Some(file) = pending.pop_front() {
            if file == end {
                let mut path = vec![file.clone()];
                let mut step = file;
                while let Some(before) = previous.get(&step) {
                    path.push(before.clone());
                    step = before.clone();
                }
                path.reverse();
                return Some(path);
            }
            for edge in self.imports.get(&file).into_iter().flatten() {
                if seen.insert(edge.module.clone()) {
                    previous.insert(edge.module.clone(), file.clone());
                    pending.push_back(edge.module.clone());
                }
            }
        }
        None
    }
}

/// The outline of each typed file, read once.
struct Outlines<'sources> {
    sources: &'sources dyn SourceFiles,
    read: HashMap<PathBuf, Option<ModuleOutline>>,
}

impl Outlines<'_> {
    /// `None` for a plain file or one that cannot be read.
    fn of(&mut self, file: &Path) -> Option<ModuleOutline> {
        let sources = self.sources;
        self.read
            .entry(file.to_path_buf())
            .or_insert_with(|| {
                if dialect_of(file) != Some(Dialect::Typed) {
                    return None;
                }
                sources.read(file).ok().map(|source| module_outline(&source))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::source_files::Disk;

    /// Writes `files` (path, source) under a new directory.
    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = directory.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        directory
    }

    fn messages(graph: &ModuleGraph, file: &Path) -> Vec<String> {
        graph.diagnostics_of(file).iter().map(|diagnostic| diagnostic.message.clone()).collect()
    }

    #[test]
    fn an_import_of_a_module_is_followed() {
        let project = project(&[
            ("bots/trend.ths", "import { a } from \"../math/averages\"\nLog(a)"),
            ("math/averages.ths", "export local a = 1"),
        ]);
        let bot = project.path().join("bots/trend.ths");
        let graph = ModuleGraph::build(std::slice::from_ref(&bot), &Disk);
        assert_eq!(messages(&graph, &bot), Vec::<String>::new());
        assert_eq!(graph.imports[&normalized(&bot)][0].module, normalized(&project.path().join("math/averages.ths")));
    }

    #[test]
    fn a_path_that_leads_nowhere_is_reported() {
        let project =
            project(&[("bot.ths", "import { a } from \"./missing\"\nimport { b } from \"math/b\"\nLog(a .. b)")]);
        let bot = project.path().join("bot.ths");
        let graph = ModuleGraph::build(std::slice::from_ref(&bot), &Disk);
        let reported = messages(&graph, &bot);
        assert!(reported[0].starts_with("Cannot find the module '"), "{reported:?}");
        assert_eq!(reported[1], "An import path starts with './' or '../': it is relative to the importing file.");
    }

    #[test]
    fn a_file_with_no_exports_cannot_be_imported() {
        let project = project(&[("bot.ths", "import { a } from \"./other\"\nLog(a)"), ("other.ths", "Log(1)")]);
        let bot = project.path().join("bot.ths");
        let graph = ModuleGraph::build(std::slice::from_ref(&bot), &Disk);
        assert_eq!(
            messages(&graph, &bot),
            ["'./other' has no exports, so it cannot be imported: only a module, a file with an export, can be."]
        );
    }

    #[test]
    fn a_circular_import_is_reported_at_each_file_of_the_cycle() {
        let project = project(&[
            ("bot.ths", "import { a } from \"./a\"\nLog(a)"),
            ("a.ths", "import { b } from \"./b\"\nexport local a = b"),
            ("b.ths", "import { a } from \"./a\"\nexport local b = 1"),
        ]);
        let bot = project.path().join("bot.ths");
        let graph = ModuleGraph::build(std::slice::from_ref(&bot), &Disk);
        assert_eq!(messages(&graph, &bot), Vec::<String>::new());
        let first_member_messages = messages(&graph, &project.path().join("a.ths"));
        let second_member_messages = messages(&graph, &project.path().join("b.ths"));
        assert_eq!(first_member_messages.len(), 1, "{first_member_messages:?}");
        assert!(first_member_messages[0].starts_with("Circular import: "), "{first_member_messages:?}");
        assert_eq!(second_member_messages.len(), 1, "{second_member_messages:?}");
    }
}
