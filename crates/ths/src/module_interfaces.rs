//! What each module of the graph gives the files that import it, worked
//! out in dependency order: a module is checked after the modules it
//! imports. A module on an import cycle has no interface (the cycle is
//! reported by `module_graph`), so imports of it are left `any`.
//!
//! Checking a module here only yields its interface; its diagnostics are
//! reported when the module itself is among the checked files.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ths_check::{CheckOptions, ImportedModules, ModuleInterface, check_module};
use ths_project::file_extensions;
use ths_syntax::Dialect;

use crate::module_graph::ModuleGraph;
use crate::project_declarations::ProjectDeclarations;
use crate::shown_paths::{shown, slash_separated};
use crate::source_files::SourceFiles;

/// What checking a module for its interface sees.
pub struct InterfaceContext<'project> {
    pub declarations: &'project ProjectDeclarations,
    pub options: CheckOptions,
    pub sources: &'project dyn SourceFiles,
}

#[derive(Debug, Default)]
pub struct ModuleInterfaces {
    /// `None` for a module on a cycle or one that cannot be read.
    by_file: HashMap<PathBuf, Option<ModuleInterface>>,
}

impl ModuleInterfaces {
    /// The interface of every module `graph` reaches.
    pub fn build(graph: &ModuleGraph, context: &InterfaceContext) -> ModuleInterfaces {
        let mut interfaces = ModuleInterfaces::default();
        let mut visiting = HashSet::new();
        for module in graph.modules() {
            interfaces.work_out(module, graph, context, &mut visiting);
        }
        interfaces
    }

    fn work_out(
        &mut self,
        module: &Path,
        graph: &ModuleGraph,
        context: &InterfaceContext,
        visiting: &mut HashSet<PathBuf>,
    ) {
        if self.by_file.contains_key(module) || !visiting.insert(module.to_path_buf()) {
            return;
        }
        for import in graph.imports_of(module) {
            self.work_out(&import.module, graph, context, visiting);
        }
        visiting.remove(module);
        let interface = context.sources.read(module).ok().and_then(|source| {
            let imported = self.imported_by(module, graph);
            check_module(
                &source,
                Dialect::Typed,
                &context.declarations.for_file(module),
                context.options,
                &imported,
                &module_name(module),
            )
            .interface
        });
        self.by_file.insert(module.to_path_buf(), interface);
    }

    /// The interfaces of the modules `file` imports, by the path its
    /// imports write.
    pub fn imported_by<'a>(&'a self, file: &Path, graph: &ModuleGraph) -> ImportedModules<'a> {
        let mut imported = ImportedModules::default();
        for import in graph.imports_of(file) {
            if let Some(Some(interface)) = self.by_file.get(&import.module) {
                imported.insert(&import.import.path, interface);
            }
        }
        imported
    }
}

/// How the types of `module` are named in importing files:
/// `import("<name>").Prices`, the name being the path shown to the owner
/// without its extension.
pub fn module_name(module: &Path) -> String {
    let shown = slash_separated(&shown(module));
    shown.strip_suffix(file_extensions::TYPED_SOURCE).map(str::to_string).unwrap_or(shown)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ths_check::{Declarations, Severity};

    use super::*;
    use crate::project_declarations;
    use crate::source_files::Disk;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = directory.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        directory
    }

    /// The errors in `file`, checked as `ths check` checks it.
    fn errors(file: &Path) -> Vec<String> {
        let files = vec![file.to_path_buf()];
        let declarations = project_declarations::load(None, &files, &Disk).unwrap();
        let graph = ModuleGraph::build(&files, &Disk);
        let context =
            InterfaceContext { declarations: &declarations, options: CheckOptions::default(), sources: &Disk };
        let interfaces = ModuleInterfaces::build(&graph, &context);
        let source = fs::read_to_string(file).unwrap();
        let imported = interfaces.imported_by(file, &graph);
        check_module(&source, Dialect::Typed, &Declarations::default(), CheckOptions::default(), &imported, "bot")
            .diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    #[test]
    fn a_type_reaches_the_bot_through_two_modules() {
        let project = project(&[
            ("bots/trend.ths", "import { latest } from \"../quotes\"\nlocal bid: string = latest().bid"),
            (
                "quotes.ths",
                "import { type Quote } from \"./math/prices\"\nexport local function latest(): Quote\n    return { bid = 1, ask = 2 }\nend",
            ),
            ("math/prices.ths", "export interface Quote { bid: number, ask: number }"),
        ]);
        assert_eq!(
            errors(&project.path().join("bots/trend.ths")),
            ["Type 'number' is not assignable to type 'string'."]
        );
    }

    #[test]
    fn a_module_on_a_cycle_is_imported_as_any() {
        let project = project(&[
            ("bot.ths", "import { a } from \"./a\"\nlocal text: string = a"),
            ("a.ths", "import { b } from \"./b\"\nexport local a = b"),
            ("b.ths", "import { a } from \"./a\"\nexport local b = a"),
        ]);
        assert_eq!(errors(&project.path().join("bot.ths")), Vec::<String>::new());
    }
}
