//! The rules of imports and exports that one file's syntax settles
//! (recorded in docs/design.md, "Imports"). Every rule starts
//! strict: no typed or bundled script exists on a server yet.
//!
//! - Imports and exports are allowed only at the top level of a file, and
//!   imports come before every other statement.
//! - A module (a file with an `export`) holds only declarations and imports
//!   at its top level: it runs before every script that imports it, on
//!   every tick, so a statement there would be a hidden effect.
//! - A module may not call `DefineCommand`, `DefineParameter`,
//!   `DefineOutput` or `DefineOutputIndex`: a command's interface stays in
//!   its own file.
//! - Each name is imported once, and no import shares a name with a
//!   top-level declaration. Each name is exported once, and an export list
//!   names a top-level declaration of the file, not an import.

use std::collections::HashSet;

use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::command_signature::{DEFINE_COMMAND, DEFINE_OUTPUT, DEFINE_OUTPUT_INDEX, DEFINE_PARAMETER, callee_name};
use crate::diagnostic::DiagnosticSink;
use crate::module_syntax::{
    declared_names, exported_names, import_statements, imported_names, is_module, top_level_statements,
};
use crate::syntax_access::child;

const INTERFACE_DEFINITIONS: [&str; 4] = [DEFINE_COMMAND, DEFINE_PARAMETER, DEFINE_OUTPUT, DEFINE_OUTPUT_INDEX];

pub(crate) fn report(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    report_placement(root, sink);
    if is_module(root) {
        report_module_statements(root, sink);
        report_interface_definitions(root, sink);
    }
    report_imported_names(root, sink);
    report_exported_names(root, sink);
}

/// Imports and exports at the top level only, imports first.
fn report_placement(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    let top_level = child(root, SyntaxKind::Block);
    for node in root.descendants().filter(|node| is_import_or_export(node.kind())) {
        if node.parent() != top_level {
            sink.error(
                node.text_range(),
                "Imports and exports are allowed only at the top level of a file.".to_string(),
            );
        }
    }
    let mut seen_other = false;
    for statement in top_level_statements(root) {
        match statement.kind() {
            SyntaxKind::ImportStatement if seen_other => {
                sink.error(statement.text_range(), "Imports must come before every other statement.".to_string());
            }
            SyntaxKind::ImportStatement | SyntaxKind::EmptyStatement => {}
            _ => seen_other = true,
        }
    }
}

fn is_import_or_export(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::ImportStatement | SyntaxKind::ExportDeclaration | SyntaxKind::ExportListStatement)
}

fn report_module_statements(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    for statement in top_level_statements(root).filter(|statement| !is_declaration(statement.kind())) {
        sink.error(
            statement.text_range(),
            "A module may hold only declarations and imports at its top level: this statement would run on every tick of every script that imports it.".to_string(),
        );
    }
}

fn is_declaration(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LocalStatement
            | SyntaxKind::LocalFunctionStatement
            | SyntaxKind::TypeAliasStatement
            | SyntaxKind::InterfaceStatement
            | SyntaxKind::DeclareFunctionStatement
            | SyntaxKind::DeclareVariableStatement
            | SyntaxKind::DeclareTypeStatement
            | SyntaxKind::ImportStatement
            | SyntaxKind::ExportDeclaration
            | SyntaxKind::ExportListStatement
            | SyntaxKind::EmptyStatement
    )
}

fn report_interface_definitions(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    for call in root.descendants().filter(|node| node.kind() == SyntaxKind::CallExpression) {
        let Some(callee) = callee_name(&call) else { continue };
        if INTERFACE_DEFINITIONS.contains(&callee.as_str()) {
            sink.error(
                call.text_range(),
                format!(
                    "A module may not call {callee}: a custom command's interface belongs in the command's own file."
                ),
            );
        }
    }
}

/// The names the file's own top-level declarations introduce.
fn top_level_names(root: &SyntaxNode) -> HashSet<String> {
    top_level_statements(root)
        .flat_map(|statement| declared_names(&statement))
        .map(|name| name.text().to_string())
        .collect()
}

fn report_imported_names(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    let declared = top_level_names(root);
    let mut imported = HashSet::new();
    for name in import_statements(root).flat_map(|statement| imported_names(&statement)) {
        let text = name.text().to_string();
        if declared.contains(&text) {
            sink.error(name.text_range(), format!("Import '{text}' has the same name as a declaration in this file."));
        } else if !imported.insert(text.clone()) {
            sink.error(name.text_range(), format!("Duplicate import '{text}'."));
        }
    }
}

fn report_exported_names(root: &SyntaxNode, sink: &mut DiagnosticSink) {
    let declared = top_level_names(root);
    let imported: HashSet<String> = import_statements(root)
        .flat_map(|statement| imported_names(&statement))
        .map(|name| name.text().to_string())
        .collect();
    let mut exported = HashSet::new();
    for name in exported_names(root) {
        let local = name.local.text();
        if imported.contains(local) {
            sink.error(
                name.local.text_range(),
                format!("Cannot export the import '{local}': re-exports are not supported."),
            );
        } else if !declared.contains(local) {
            sink.error(
                name.local.text_range(),
                format!("Cannot export '{local}': nothing at the top level of this file has that name."),
            );
        }
        let text = name.exported.text().to_string();
        if !exported.insert(text.clone()) {
            sink.error(name.exported.text_range(), format!("Duplicate export '{text}'."));
        }
    }
}
