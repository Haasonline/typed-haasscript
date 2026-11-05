//! The diagnostics of one file, seeing the TradeServer's built-in
//! declarations, the project's (see `project_declarations`) and the
//! modules it imports. `ths check` prints them; the language server sends
//! them to the editor, with the file's analysis for hover, definitions and
//! completion.

use std::path::Path;

use ths_check::{CheckOptions, Declarations, Diagnostic, FileAnalysis, ImportedModules, analyze_module, check_module};
use ths_syntax::Dialect;

use crate::module_graph::ModuleGraph;
use crate::module_interfaces::{ModuleInterfaces, module_name};
use crate::project_declarations::ProjectDeclarations;

/// What every checked file sees.
pub struct Context<'project> {
    pub declarations: &'project ProjectDeclarations,
    pub modules: &'project ModuleGraph,
    pub interfaces: &'project ModuleInterfaces,
    pub options: CheckOptions,
}

/// Every diagnostic of `file`, whose text is `source`, in source order.
pub fn diagnostics(file: &Path, source: &str, dialect: Dialect, context: &Context) -> Vec<Diagnostic> {
    let checked = checked_with(file, context, |declarations, imported, name| {
        check_module(source, dialect, declarations, context.options, imported, name)
    });
    with_import_diagnostics(file, checked.diagnostics, context)
}

/// The diagnostics of `file`, as `diagnostics` gives them, and its analysis.
pub fn analyzed(file: &Path, source: &str, dialect: Dialect, context: &Context) -> (Vec<Diagnostic>, FileAnalysis) {
    let analyzed = checked_with(file, context, |declarations, imported, name| {
        analyze_module(source, dialect, declarations, context.options, imported, name)
    });
    (with_import_diagnostics(file, analyzed.checked.diagnostics, context), analyzed.analysis)
}

/// Runs `check` with what `file` sees.
fn checked_with<Checked>(
    file: &Path,
    context: &Context,
    check: impl FnOnce(&Declarations, &ImportedModules, &str) -> Checked,
) -> Checked {
    let imported = context.interfaces.imported_by(file, context.modules);
    let declarations = context.declarations.for_file(file);
    check(&declarations, &imported, &module_name(file))
}

/// Adds what is wrong with `file`'s imports, and sorts by position.
fn with_import_diagnostics(file: &Path, mut diagnostics: Vec<Diagnostic>, context: &Context) -> Vec<Diagnostic> {
    diagnostics.extend(context.modules.diagnostics_of(file).iter().cloned());
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start());
    diagnostics
}
