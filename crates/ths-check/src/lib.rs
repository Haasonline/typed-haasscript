//! Type checker for Typed HaasScript and plain HaasScript.

mod analysis;
mod ascii_only;
mod assignability;
mod checker;
mod command_declaration;
mod command_interface_types;
mod command_signature;
mod comment_and_string_interiors;
mod declarations;
mod diagnostic;
mod documentation;
mod engine_types;
mod enum_categories;
mod inference;
mod literals;
mod module_interface;
mod module_rules;
mod module_syntax;
mod narrowing;
mod primitive_type_names;
mod scopes;
mod spreading_casts;
mod syntax_access;
mod type_environment;
mod types;
mod visibility;

pub use analysis::{
    AnalyzedSymbol, FileAnalysis, ImportOrigin, Member, MemberAccess, MemberSite, Reference, ReferenceTarget,
    SymbolKind,
};
pub use declarations::Declarations;
pub use diagnostic::{Diagnostic, Severity};
pub use engine_types::SERIES_TYPE;
pub use module_interface::{ImportedModules, ModuleInterface};
pub use module_syntax::{
    ExportedValue, ImportReference, ImportedValue, ModuleImport, ModuleOutline, export_definition, module_outline,
};
use ths_syntax::{Dialect, parse};

use crate::checker::{Checker, Rules};
use crate::diagnostic::DiagnosticSink;
use crate::type_environment::TypeEnvironment;

/// Settings that change what a check reports.
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckOptions {
    /// Report implicit `any`: unannotated parameters and variables whose
    /// type cannot be inferred. Typed files only, since a plain file cannot
    /// hold annotations.
    pub strict: bool,
    /// Warn where a name read falls back to an outer or global definition
    /// because a `local` of the same name ended with its block. Often
    /// intended, so off by default.
    pub shadowing: bool,
}

/// The declaration a custom command gives the scripts that call it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDeclaration {
    /// The name callers use: `CC_` and the `DefineCommand` name.
    pub command: String,
    /// Typed HaasScript declarations, for `Declarations::with_sources`.
    pub text: String,
}

/// The declaration of the custom command `source` defines, read from its
/// `Define…` calls, with the result types checked against
/// `declarations`; `None` when it defines none.
pub fn command_declaration(source: &str, dialect: Dialect, declarations: &Declarations) -> Option<CommandDeclaration> {
    let root = parse(source, dialect).syntax_node();
    let signature = command_signature::read(&root)?;
    let interface = command_interface_types::infer(&signature, &root, dialect, declarations);
    let text = command_declaration::render(&signature, &interface)?;
    let command = format!("{}{}", command_declaration::CUSTOM_COMMAND_PREFIX, signature.name);
    Some(CommandDeclaration { command, text })
}

/// Every syntax and type diagnostic for `source`, in source order, seeing
/// only the script's own declarations.
pub fn check(source: &str, dialect: Dialect) -> Vec<Diagnostic> {
    check_with(source, dialect, &Declarations::default(), CheckOptions::default())
}

/// Every syntax and type diagnostic for `source`, in source order, seeing
/// `declarations` as well as the script's own. A plain file is out of scope
/// for type corrections: it is read only for the types a typed file can
/// use, so it reports only what the server rejects, syntax errors, unknown
/// names and globals a custom command creates, which stop that. Renaming it
/// to `.ths` shows everything.
pub fn check_with(
    source: &str,
    dialect: Dialect,
    declarations: &Declarations,
    options: CheckOptions,
) -> Vec<Diagnostic> {
    check_module(source, dialect, declarations, options, &ImportedModules::default(), "").diagnostics
}

/// A checked file: its diagnostics, and for a module what it gives the
/// files that import it.
#[derive(Debug)]
pub struct CheckedFile {
    pub diagnostics: Vec<Diagnostic>,
    /// `None` for a file without exports.
    pub interface: Option<ModuleInterface>,
}

/// Checks `source` as `check_with` does, seeing the modules it imports in
/// `modules`. `module_name` names the file in the types other files see
/// from it (`import("<module_name>").Prices`).
pub fn check_module(
    source: &str,
    dialect: Dialect,
    declarations: &Declarations,
    options: CheckOptions,
    modules: &ImportedModules,
    module_name: &str,
) -> CheckedFile {
    let sink = match dialect {
        Dialect::Typed => DiagnosticSink::default(),
        Dialect::Plain => DiagnosticSink::only_rejections(),
    };
    diagnose_into(sink, source, dialect, declarations, options, modules, module_name, Recording::DiagnosticsOnly).0
}

/// A file checked for an editor: what `check_module` gives, and what the
/// editor asks of it (see `FileAnalysis`).
#[derive(Debug)]
pub struct AnalyzedFile {
    pub checked: CheckedFile,
    pub analysis: FileAnalysis,
}

/// Checks `source` as `check_module` does, recording its analysis too.
pub fn analyze_module(
    source: &str,
    dialect: Dialect,
    declarations: &Declarations,
    options: CheckOptions,
    modules: &ImportedModules,
    module_name: &str,
) -> AnalyzedFile {
    let sink = match dialect {
        Dialect::Typed => DiagnosticSink::default(),
        Dialect::Plain => DiagnosticSink::only_rejections(),
    };
    let (checked, analysis) =
        diagnose_into(sink, source, dialect, declarations, options, modules, module_name, Recording::WithAnalysis);
    AnalyzedFile { checked, analysis: analysis.unwrap_or_default() }
}

/// Whether a check also records the file's analysis.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recording {
    DiagnosticsOnly,
    WithAnalysis,
}

/// Every diagnostic in either dialect, including the ones `check_with`
/// leaves out for a plain file.
#[cfg(test)]
pub(crate) fn diagnose(
    source: &str,
    dialect: Dialect,
    declarations: &Declarations,
    options: CheckOptions,
) -> Vec<Diagnostic> {
    diagnose_into(
        DiagnosticSink::default(),
        source,
        dialect,
        declarations,
        options,
        &ImportedModules::default(),
        "",
        Recording::DiagnosticsOnly,
    )
    .0
    .diagnostics
}

#[allow(clippy::too_many_arguments)]
fn diagnose_into(
    mut sink: DiagnosticSink,
    source: &str,
    dialect: Dialect,
    declarations: &Declarations,
    options: CheckOptions,
    modules: &ImportedModules,
    module_name: &str,
    recording: Recording,
) -> (CheckedFile, Option<FileAnalysis>) {
    let parsed = parse(source, dialect);
    for error in &parsed.errors {
        sink.rejection(error.range, error.message.clone());
    }
    ascii_only::report_non_ascii(source, &mut sink);
    let root = parsed.syntax_node();
    if dialect == Dialect::Typed {
        module_rules::report(&root, &mut sink);
    }
    let imported = module_interface::imported_declarations(&root, modules);
    let environment = TypeEnvironment::collect(declarations.roots(), &imported, &root, &mut sink);
    let rules = Rules {
        strict: options.strict && dialect == Dialect::Typed,
        shadowing: options.shadowing,
        unknown_names: declarations.covers_server_names(),
        lenient: dialect == Dialect::Plain,
    };
    let checker = Checker::new(&environment, sink, rules).with_modules(modules);
    let checker = match recording {
        Recording::DiagnosticsOnly => checker,
        Recording::WithAnalysis => checker.with_analysis(),
    };
    let checked = checker.check_root(declarations.roots(), &root);
    let interface = module_syntax::is_module(&root)
        .then(|| module_interface::build(&root, module_name, &environment, &checked.exported_values, modules));
    let mut diagnostics = checked.sink.diagnostics;
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start());
    let analysis = checked.analysis.map(|analysis| FileAnalysis {
        comment_and_string_interiors: comment_and_string_interiors::interiors(&root),
        ..analysis
    });
    (CheckedFile { diagnostics, interface }, analysis)
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod analysis_tests;

#[cfg(test)]
mod built_in_declaration_tests;

#[cfg(test)]
mod cast_tests;

#[cfg(test)]
mod command_argument_tests;

#[cfg(test)]
mod custom_command_tests;

#[cfg(test)]
mod dialect_tests;

#[cfg(test)]
mod function_tests;

#[cfg(test)]
mod generic_tests;

#[cfg(test)]
mod indexing_tests;

#[cfg(test)]
mod interface_and_table_tests;

#[cfg(test)]
mod known_name_tests;

#[cfg(test)]
mod module_import_tests;

#[cfg(test)]
mod multiple_return_tests;

#[cfg(test)]
mod nil_tests;

#[cfg(test)]
mod opaque_type_tests;

#[cfg(test)]
mod operator_tests;

#[cfg(test)]
mod series_tests;

#[cfg(test)]
mod variable_tests;
