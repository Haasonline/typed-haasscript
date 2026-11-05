//! Which modules a trade script's bundle holds, and in what order: every
//! module it imports a value from, directly or not, each after the modules
//! it imports values from. A statement importing only types adds nothing,
//! since types do not exist at run time.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ths_check::{ModuleOutline, module_outline};
use ths_project::{normalized, resolve_import};

use crate::shown_paths::shown;

/// A typed file read for bundling.
#[derive(Debug)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub source: String,
    pub outline: ModuleOutline,
    /// For each import with values, in source order, the index of its
    /// module in `BundlePlan::modules`.
    pub imported_modules: Vec<usize>,
}

#[derive(Debug)]
pub struct BundlePlan {
    /// In dependency order.
    pub modules: Vec<PlannedFile>,
    pub script: PlannedFile,
}

/// Reads `file` and every module it needs; `None` when `file` is itself a
/// module, which is not bundled. Fails on an import that leads nowhere, on
/// a file that is not a module, and on a cycle; `ths check` reports each
/// at the import.
pub fn plan(file: &Path) -> Result<Option<BundlePlan>, String> {
    let file = normalized(file);
    let (source, outline) = read(&file)?;
    if outline.is_module {
        return Ok(None);
    }
    let mut planner = Planner::default();
    let script = planner.plan_imports(&file, source, outline)?;
    Ok(Some(BundlePlan { modules: planner.modules, script }))
}

fn read(file: &Path) -> Result<(String, ModuleOutline), String> {
    let source =
        std::fs::read_to_string(file).map_err(|error| format!("{}: cannot read: {error}", shown(file).display()))?;
    let outline = module_outline(&source);
    Ok((source, outline))
}

#[derive(Default)]
struct Planner {
    modules: Vec<PlannedFile>,
    index_of: HashMap<PathBuf, usize>,
    visiting: HashSet<PathBuf>,
}

impl Planner {
    /// Plans every module `file` imports values from.
    fn plan_imports(&mut self, file: &Path, source: String, outline: ModuleOutline) -> Result<PlannedFile, String> {
        let shown_file = shown(file);
        self.visiting.insert(file.to_path_buf());
        let mut imported_modules = Vec::new();
        for import in outline.imports.iter().filter(|import| !import.values.is_empty()) {
            let written = &import.reference.path;
            let module = resolve_import(file, written)
                .map_err(|error| format!("{}: cannot bundle '{written}': {error}", shown_file.display()))?;
            imported_modules.push(self.module_index(&module, file, written)?);
        }
        self.visiting.remove(file);
        Ok(PlannedFile { path: file.to_path_buf(), source, outline, imported_modules })
    }

    fn module_index(&mut self, module: &Path, importing: &Path, written: &str) -> Result<usize, String> {
        if let Some(index) = self.index_of.get(module) {
            return Ok(*index);
        }
        let importing_shown = shown(importing);
        if self.visiting.contains(module) {
            return Err(format!("{}: cannot bundle '{written}': circular import", importing_shown.display()));
        }
        let (source, outline) = read(module)?;
        if !outline.is_module {
            return Err(format!(
                "{}: cannot bundle '{written}': it has no exports, so it is not a module",
                importing_shown.display()
            ));
        }
        let planned = self.plan_imports(module, source, outline)?;
        self.modules.push(planned);
        let index = self.modules.len() - 1;
        self.index_of.insert(module.to_path_buf(), index);
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = directory.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        directory
    }

    fn file_names(plan: &BundlePlan) -> Vec<String> {
        plan.modules.iter().map(|module| module.path.file_name().unwrap().to_string_lossy().to_string()).collect()
    }

    #[test]
    fn modules_come_after_the_modules_they_import() {
        let project = project(&[
            ("bot.ths", "import { b } from \"./b\"\nimport { a } from \"./a\"\nLog(a .. b)"),
            ("a.ths", "export local a = 1"),
            ("b.ths", "import { a } from \"./a\"\nexport local b = a"),
        ]);
        let plan = plan(&project.path().join("bot.ths")).unwrap().unwrap();
        assert_eq!(file_names(&plan), ["a.ths", "b.ths"]);
        assert_eq!(plan.script.imported_modules, [1, 0]);
        assert_eq!(plan.modules[1].imported_modules, [0]);
    }

    #[test]
    fn a_type_only_import_adds_no_module() {
        let project = project(&[
            ("bot.ths", "import { type Quote } from \"./quotes\"\nlocal q: Quote = { bid = 1 }\nLog(q.bid)"),
            ("quotes.ths", "export interface Quote { bid: number }"),
        ]);
        let plan = plan(&project.path().join("bot.ths")).unwrap().unwrap();
        assert!(plan.modules.is_empty());
        assert!(plan.script.imported_modules.is_empty());
    }

    #[test]
    fn a_cycle_cannot_be_bundled() {
        let project = project(&[
            ("bot.ths", "import { a } from \"./a\"\nLog(a)"),
            ("a.ths", "import { b } from \"./b\"\nexport local a = b"),
            ("b.ths", "import { a } from \"./a\"\nexport local b = a"),
        ]);
        let error = plan(&project.path().join("bot.ths")).unwrap_err();
        assert!(error.ends_with("cannot bundle './a': circular import"), "{error}");
    }

    #[test]
    fn a_module_is_not_bundled() {
        let project = project(&[("a.ths", "import { b } from \"./missing\"\nexport local a = b")]);
        assert!(plan(&project.path().join("a.ths")).unwrap().is_none());
    }
}
