//! Imports and exports while a file is checked (docs/design.md,
//! "Imports"):
//!
//! - Each import is a read-only local holding a copy of the export, with
//!   the export's type; `* as m` holds every exported value as one table.
//!   A type is imported with `type` and a value without it.
//! - A module may not reassign a variable it exports: importers hold the
//!   copy taken when the module finished loading.
//! - An import that is never used is an error.
//!
//! An import whose path leads nowhere (the module graph reports it) binds
//! its names as `any`, so nothing else is reported about them.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::Checker;
use super::analysis_recording::Declared;
use crate::analysis::{ImportOrigin, SymbolKind};
use crate::module_syntax::{
    Import, declared_names, exported_names, import_reference, import_statements, imports_of, is_module,
    top_level_statements,
};
use crate::scopes::{ReadOnly, SymbolId};
use crate::syntax_access::first_name;
use crate::types::{ObjectType, Property, Type};

/// The imports and exports of the file being checked.
#[derive(Debug, Default)]
pub(crate) struct ModuleState {
    /// The top-level locals the file exports.
    exported_locals: HashSet<String>,
    /// Value imports not read yet.
    unread: Vec<(SymbolId, SyntaxToken)>,
    read: HashSet<SymbolId>,
    type_imports: Vec<SyntaxToken>,
}

impl Checker<'_> {
    /// Notes the top-level locals the file exports, so they become
    /// read-only once declared.
    pub(super) fn note_exported_locals(&mut self, root: &SyntaxNode) {
        if is_module(root) {
            self.module.exported_locals =
                exported_names(root).iter().map(|name| name.local.text().to_string()).collect();
        }
    }

    /// Binds every import.
    pub(super) fn bind_imports(&mut self, root: &SyntaxNode) {
        for statement in import_statements(root) {
            let Some(reference) = import_reference(&statement) else { continue };
            let interface = self.modules.and_then(|modules| modules.get(&reference.path));
            for import in imports_of(&statement) {
                // An import already reported is not reported as unused too.
                if import.is_type {
                    if self.check_type_import(&import, interface, &reference.path) {
                        self.module.type_imports.push(import.local.clone());
                    }
                    continue;
                }
                let value = match interface {
                    Some(interface) => self.imported_value(&import, interface, &reference.path),
                    None => Some(Type::Any),
                };
                let id = self.scopes.declare(import.local.text(), value.clone().unwrap_or(Type::Any));
                self.scopes.mark_read_only(id, ReadOnly::Import);
                let origin = ImportOrigin {
                    module_path: reference.path.clone(),
                    exported: import.exported.as_ref().map(|exported| exported.text().to_string()),
                };
                let declared = Declared {
                    kind: SymbolKind::Import,
                    name: Some(&import.local),
                    visible: None,
                    statement: Some(&statement),
                    import: Some(origin),
                };
                self.record_definition(id, import.local.text(), declared);
                if value.is_some() {
                    self.module.unread.push((id, import.local.clone()));
                }
            }
        }
    }

    /// The type of a value import; `None` once a missing export or a
    /// type imported as a value is reported.
    fn imported_value(&mut self, import: &Import, interface: &crate::ModuleInterface, path: &str) -> Option<Type> {
        let Some(exported) = &import.exported else {
            let properties = interface
                .values
                .iter()
                .map(|(name, value)| (name.clone(), Property { value: value.clone(), optional: false }))
                .collect();
            return Some(Type::Object(Rc::new(ObjectType { properties, index: None, open: false })));
        };
        if let Some(value) = interface.values.get(exported.text()) {
            return Some(value.clone());
        }
        let message = if interface.types.contains_key(exported.text()) {
            format!("'{}' is a type: import it with 'type'.", exported.text())
        } else {
            format!("Module '{path}' has no export '{}'.", exported.text())
        };
        self.sink.error(exported.text_range(), message);
        None
    }

    /// Whether a type import names an exported type, reporting it if not.
    fn check_type_import(&mut self, import: &Import, interface: Option<&crate::ModuleInterface>, path: &str) -> bool {
        let (Some(interface), Some(exported)) = (interface, &import.exported) else { return true };
        if interface.types.contains_key(exported.text()) {
            return true;
        }
        let message = if interface.values.contains_key(exported.text()) {
            format!("'{}' is a value: import it without 'type'.", exported.text())
        } else {
            format!("Module '{path}' has no export '{}'.", exported.text())
        };
        self.sink.error(exported.text_range(), message);
        false
    }

    /// Notes a read of the variable `id`.
    pub(super) fn note_read(&mut self, id: SymbolId) {
        if !self.module.unread.is_empty() {
            self.module.read.insert(id);
        }
    }

    /// Marks a top-level local the file exports as read-only, once
    /// declared.
    pub(super) fn note_declared_local(&mut self, name: &str, id: SymbolId) {
        if self.scopes.is_top_level() && self.module.exported_locals.contains(name) {
            self.scopes.mark_read_only(id, ReadOnly::Export);
        }
    }

    /// Reports an assignment to an import, or to a variable the module
    /// exports.
    pub(super) fn check_read_only_assignment(&mut self, name: &SyntaxToken) {
        let Some(id) = self.scopes.lookup(name.text()) else { return };
        let Some(reason) = self.scopes.read_only(id) else { return };
        // Reported here, so not also as unused.
        self.module.read.insert(id);
        let message = match reason {
            ReadOnly::Import => format!("Cannot assign to '{}': an import is read-only.", name.text()),
            ReadOnly::Export => format!(
                "Cannot assign to '{}': it is exported, and importing files would not see the new value. Share changing state through a table's fields or a function.",
                name.text()
            ),
        };
        self.sink.error(name.text_range(), message);
    }

    /// Reports every import the file never uses. A name an export list
    /// mentions counts as used (re-exporting is its own error), and a name
    /// reported as a duplicate or a clash is not reported again.
    pub(super) fn report_unused_imports(&mut self, root: &SyntaxNode) {
        let mentioned: HashSet<String> =
            exported_names(root).iter().map(|name| name.local.text().to_string()).collect();
        let clashing = clashing_names(root);
        let unread: Vec<SyntaxToken> = self
            .module
            .unread
            .iter()
            .filter(|(id, _)| !self.module.read.contains(id))
            .map(|(_, name)| name.clone())
            .collect();
        let type_names = type_names_written(root);
        let unused_types: Vec<SyntaxToken> =
            self.module.type_imports.iter().filter(|name| !type_names.contains(name.text())).cloned().collect();
        for name in unread.into_iter().chain(unused_types) {
            if mentioned.contains(name.text()) || clashing.contains(name.text()) {
                continue;
            }
            self.sink.error(name.text_range(), format!("'{}' is imported but never used.", name.text()));
        }
    }

    /// The types of the values the module exports, by local name.
    pub(super) fn exported_value_types(&self) -> HashMap<String, Type> {
        self.module
            .exported_locals
            .iter()
            .filter_map(|name| {
                let id = self.scopes.lookup(name)?;
                Some((name.clone(), self.scopes.declared_type(id).clone()))
            })
            .collect()
    }
}

/// The imported names `module_rules` reports: imported twice, or also
/// declared by the file.
fn clashing_names(root: &SyntaxNode) -> HashSet<String> {
    let declared: HashSet<String> = top_level_statements(root)
        .flat_map(|statement| declared_names(&statement))
        .map(|name| name.text().to_string())
        .collect();
    let mut seen = HashSet::new();
    let mut clashing = HashSet::new();
    for name in import_statements(root).flat_map(|statement| imports_of(&statement)) {
        let text = name.local.text().to_string();
        if declared.contains(&text) || !seen.insert(text.clone()) {
            clashing.insert(text);
        }
    }
    clashing
}

/// Every name written in a type position.
fn type_names_written(root: &SyntaxNode) -> HashSet<String> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::NameType)
        .filter_map(|node| first_name(&node))
        .map(|name| name.text().to_string())
        .collect()
}
