//! Where each type name written in a file is declared, read from syntax
//! alone, for go-to-definition. Types are kept apart from the file's value
//! symbols, which also drive completion of names in expressions.

use std::collections::HashMap;

use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::analysis::{ImportOrigin, TypeReference, TypeTarget};
use crate::module_syntax::{import_reference, import_statements, imports_of};
use crate::syntax_access::{child, children, declared_name, first_name};
use crate::type_environment::type_statements;

/// Every type name in the file whose declaration is known, in source order:
/// the names in type expressions, and the names a `type` import binds. A
/// built-in type has no declaration in the file, and is left out.
pub(crate) fn type_references(root: &SyntaxNode) -> Vec<TypeReference> {
    let declared = declared_types(root);
    let imported = imported_types(root);
    let mut references: Vec<TypeReference> = root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameType)
        .filter_map(|node| {
            let name = first_name(&node)?;
            let target = type_parameter(&node, name.text())
                .map(|parameter| TypeTarget::InFile(parameter.text_range()))
                .or_else(|| declared.get(name.text()).map(|range| TypeTarget::InFile(*range)))
                .or_else(|| imported.get(name.text()).cloned().map(TypeTarget::Imported))?;
            Some(TypeReference { range: name.text_range(), target })
        })
        .collect();
    references.extend(type_import_names(root));
    references.sort_by_key(|reference| reference.range.start());
    references
}

/// The innermost type parameter named `name` of a declaration enclosing
/// `node`.
fn type_parameter(node: &SyntaxNode, name: &str) -> Option<SyntaxToken> {
    node.ancestors().find_map(|ancestor| {
        let list = child(&ancestor, SyntaxKind::TypeParameterList)?;
        children(&list, SyntaxKind::TypeParameter)
            .filter_map(|parameter| first_name(&parameter))
            .find(|parameter| parameter.text() == name)
    })
}

/// The name each `type`, `interface` and `declare type` statement in the
/// file declares; the first declaration of a name wins.
fn declared_types(root: &SyntaxNode) -> HashMap<String, rowan::TextRange> {
    let mut declared = HashMap::new();
    for name in type_statements(root).filter_map(|statement| declared_name(&statement)) {
        declared.entry(name.text().to_string()).or_insert(name.text_range());
    }
    declared
}

/// The module export each imported name stands for, by its local name.
fn imported_types(root: &SyntaxNode) -> HashMap<String, ImportOrigin> {
    let mut imported = HashMap::new();
    for statement in import_statements(root) {
        let Some(reference) = import_reference(&statement) else { continue };
        for import in imports_of(&statement) {
            let origin = ImportOrigin {
                module_path: reference.path.clone(),
                exported: import.exported.map(|exported| exported.text().to_string()),
            };
            imported.entry(import.local.text().to_string()).or_insert(origin);
        }
    }
    imported
}

/// The names in `type` import specifiers, which lead to the export they
/// import: the exported name and the alias alike. Value imports are
/// references of the file's value symbols already.
fn type_import_names(root: &SyntaxNode) -> Vec<TypeReference> {
    let mut references = Vec::new();
    for statement in import_statements(root) {
        let Some(reference) = import_reference(&statement) else { continue };
        for import in imports_of(&statement).into_iter().filter(|import| import.is_type) {
            let Some(exported) = import.exported else { continue };
            let origin =
                ImportOrigin { module_path: reference.path.clone(), exported: Some(exported.text().to_string()) };
            references
                .push(TypeReference { range: exported.text_range(), target: TypeTarget::Imported(origin.clone()) });
            if import.local != exported {
                references
                    .push(TypeReference { range: import.local.text_range(), target: TypeTarget::Imported(origin) });
            }
        }
    }
    references
}
