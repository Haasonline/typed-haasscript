//! Reading a typed file's imports and exports from its syntax tree.

use std::collections::HashSet;

use rowan::TextRange;
use ths_syntax::{Dialect, SyntaxKind, SyntaxNode, SyntaxToken, parse};

use crate::literals::literal_type;
use crate::syntax_access::{child, children, declared_name, first_name, token, tokens};
use crate::types::Type;

/// The path an import names, as written, and where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReference {
    pub path: String,
    pub range: TextRange,
}

/// A value an import binds at run time; a `type` import binds none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedValue {
    /// The name in the importing file.
    pub local: String,
    /// The export it copies; `None` for `* as name`, the whole exports
    /// table.
    pub exported: Option<String>,
}

/// One import statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImport {
    pub reference: ImportReference,
    /// Empty when the statement imports only types.
    pub values: Vec<ImportedValue>,
}

/// A value a module exports: its name in the exports table, and the local
/// that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedValue {
    pub exported: String,
    pub local: String,
}

/// What resolving and bundling a typed file's imports needs to know about
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOutline {
    /// A file with an `export` is a module; one without is a trade script.
    pub is_module: bool,
    pub imports: Vec<ModuleImport>,
    /// Types are left out: they do not exist at run time.
    pub exported_values: Vec<ExportedValue>,
}

/// The outline of a typed source.
pub fn module_outline(source: &str) -> ModuleOutline {
    let root = parse(source, Dialect::Typed).syntax_node();
    let imports = import_statements(&root)
        .filter_map(|statement| {
            let reference = import_reference(&statement)?;
            let values = imports_of(&statement)
                .into_iter()
                .filter(|import| !import.is_type)
                .map(|import| ImportedValue {
                    local: import.local.text().to_string(),
                    exported: import.exported.map(|exported| exported.text().to_string()),
                })
                .collect();
            Some(ModuleImport { reference, values })
        })
        .collect();
    ModuleOutline { is_module: is_module(&root), imports, exported_values: exported_values(&root) }
}

/// Where a typed module declares what it exports as `exported`: the name
/// in its declaration, or the export list entry when no top-level
/// declaration names the local.
pub fn export_definition(source: &str, exported: &str) -> Option<TextRange> {
    let root = parse(source, Dialect::Typed).syntax_node();
    let name = exported_names(&root).into_iter().find(|name| name.exported.text() == exported)?;
    let declared = top_level_statements(&root)
        .flat_map(|statement| declared_names(&statement))
        .find(|declared| declared.text() == name.local.text());
    Some(declared.unwrap_or(name.local).text_range())
}

/// The exported names whose local is a value, not a type.
fn exported_values(root: &SyntaxNode) -> Vec<ExportedValue> {
    let types: HashSet<String> = top_level_statements(root)
        .filter(declares_types)
        .flat_map(|statement| declared_names(&statement))
        .map(|name| name.text().to_string())
        .collect();
    exported_names(root)
        .into_iter()
        .filter(|name| !types.contains(name.local.text()))
        .map(|name| ExportedValue { exported: name.exported.text().to_string(), local: name.local.text().to_string() })
        .collect()
}

/// A `type` or `interface` declaration, exported or not.
fn declares_types(statement: &SyntaxNode) -> bool {
    let declaration = match statement.kind() {
        SyntaxKind::ExportDeclaration => statement.first_child(),
        _ => Some(statement.clone()),
    };
    declaration.is_some_and(|declaration| {
        matches!(declaration.kind(), SyntaxKind::TypeAliasStatement | SyntaxKind::InterfaceStatement)
    })
}

/// Whether the file exports anything.
pub(crate) fn is_module(root: &SyntaxNode) -> bool {
    root.descendants()
        .any(|node| matches!(node.kind(), SyntaxKind::ExportDeclaration | SyntaxKind::ExportListStatement))
}

/// The statements of the file's own block, in order.
pub(crate) fn top_level_statements(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    child(root, SyntaxKind::Block).into_iter().flat_map(|block| block.children())
}

pub(crate) fn import_statements(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    root.descendants().filter(|node| node.kind() == SyntaxKind::ImportStatement)
}

/// The path of an import statement, without its quotes.
pub(crate) fn import_reference(statement: &SyntaxNode) -> Option<ImportReference> {
    let path = token(statement, SyntaxKind::String)?;
    match literal_type(&path)? {
        Type::StringLiteral(text) => Some(ImportReference { path: text, range: path.text_range() }),
        _ => None,
    }
}

/// A name an import makes known in the importing file: the alias when
/// there is one.
pub(crate) fn imported_names(statement: &SyntaxNode) -> Vec<SyntaxToken> {
    imports_of(statement).into_iter().map(|import| import.local).collect()
}

/// What one import statement binds.
#[derive(Debug, Clone)]
pub(crate) struct Import {
    /// The name in the importing file: the alias when there is one.
    pub(crate) local: SyntaxToken,
    /// The export it names; `None` for `* as m`, which binds every export.
    pub(crate) exported: Option<SyntaxToken>,
    /// Marked `type`: a type, which the bundle leaves out.
    pub(crate) is_type: bool,
}

const TYPE_MODIFIER: &str = "type";
/// `type T as U`: the modifier, the name, `as` and the alias.
const TYPE_IMPORT_WITH_ALIAS: usize = 4;
/// `type T`.
const TYPE_IMPORT: usize = 2;

pub(crate) fn imports_of(statement: &SyntaxNode) -> Vec<Import> {
    if let Some(namespace) = child(statement, SyntaxKind::NamespaceImport) {
        let local = tokens(&namespace).filter(|token| token.kind() == SyntaxKind::Identifier).last();
        return local.map(|local| Import { local, exported: None, is_type: false }).into_iter().collect();
    }
    children(statement, SyntaxKind::ImportSpecifier)
        .filter_map(|specifier| {
            let names: Vec<SyntaxToken> =
                tokens(&specifier).filter(|token| token.kind() == SyntaxKind::Identifier).collect();
            // `type as U` imports the name `type`: three names, not two or four.
            let is_type =
                matches!(names.len(), TYPE_IMPORT | TYPE_IMPORT_WITH_ALIAS) && names[0].text() == TYPE_MODIFIER;
            let named = if is_type { &names[1..] } else { &names[..] };
            Some(Import { exported: Some(named.first()?.clone()), local: named.last()?.clone(), is_type })
        })
        .collect()
}

/// A name a module exports, and the token that names it.
#[derive(Debug, Clone)]
pub(crate) struct ExportedName {
    pub(crate) exported: SyntaxToken,
    /// For an export list entry, the local it names; for a declaration,
    /// the declared name itself.
    pub(crate) local: SyntaxToken,
}

/// Every name the file exports at its top level, in source order; an
/// export anywhere else is an error of its own.
pub(crate) fn exported_names(root: &SyntaxNode) -> Vec<ExportedName> {
    let mut names = Vec::new();
    for statement in top_level_statements(root) {
        match statement.kind() {
            SyntaxKind::ExportDeclaration => {
                for declared in declared_names(&statement) {
                    names.push(ExportedName { exported: declared.clone(), local: declared });
                }
            }
            SyntaxKind::ExportListStatement => {
                for specifier in children(&statement, SyntaxKind::ExportSpecifier) {
                    let parts: Vec<SyntaxToken> =
                        tokens(&specifier).filter(|token| token.kind() == SyntaxKind::Identifier).collect();
                    if let (Some(local), Some(exported)) = (parts.first(), parts.last()) {
                        names.push(ExportedName { exported: exported.clone(), local: local.clone() });
                    }
                }
            }
            _ => {}
        }
    }
    names
}

/// The names a top-level declaration introduces: every name of a `local`,
/// the name of a `local function`, `type` or `interface`.
pub(crate) fn declared_names(declaration: &SyntaxNode) -> Vec<SyntaxToken> {
    match declaration.kind() {
        SyntaxKind::LocalStatement => child(declaration, SyntaxKind::NameList)
            .map(|list| {
                children(&list, SyntaxKind::NameBinding)
                    .filter_map(|binding| token(&binding, SyntaxKind::Identifier))
                    .collect()
            })
            .unwrap_or_default(),
        SyntaxKind::LocalFunctionStatement => first_name(declaration).into_iter().collect(),
        SyntaxKind::TypeAliasStatement | SyntaxKind::InterfaceStatement => {
            declared_name(declaration).into_iter().collect()
        }
        SyntaxKind::ExportDeclaration => declaration.children().flat_map(|inner| declared_names(&inner)).collect(),
        _ => Vec::new(),
    }
}
