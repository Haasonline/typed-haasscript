//! What a module gives the files that import it: the type of each exported
//! value, and every named type those types need. In an importing file a
//! module's types are named after the module, `import("<module>").Prices`,
//! as TypeScript shows a type from another file, so two modules may both
//! export a `Prices`; a `type` import adds its own name for one of them.

use std::collections::{BTreeMap, HashMap, HashSet};

use ths_syntax::SyntaxNode;

use crate::module_syntax::{exported_names, import_reference, import_statements, imports_of};
use crate::type_environment::{NamedDeclaration, TypeEnvironment};
use crate::types::{Renaming, Type};

/// A checked module's exports.
#[derive(Debug, Clone, Default)]
pub struct ModuleInterface {
    /// Each exported value's type, by export name.
    pub(crate) values: BTreeMap<String, Type>,
    /// Each exported type's name in importing files, by export name.
    pub(crate) types: BTreeMap<String, String>,
    /// Every named type the exports need, under its name in importing
    /// files, including those the module itself imports.
    pub(crate) declarations: Vec<NamedDeclaration>,
}

/// The interfaces of the modules a file imports, by the path its imports
/// write. A path missing here leads nowhere; its imports are left `any`.
#[derive(Debug, Default)]
pub struct ImportedModules<'interfaces> {
    by_path: HashMap<String, &'interfaces ModuleInterface>,
}

impl<'interfaces> ImportedModules<'interfaces> {
    pub fn insert(&mut self, path: &str, interface: &'interfaces ModuleInterface) {
        self.by_path.insert(path.to_string(), interface);
    }

    pub(crate) fn get(&self, path: &str) -> Option<&'interfaces ModuleInterface> {
        self.by_path.get(path).copied()
    }
}

/// The name of module `module`'s type `name` in the files importing it.
fn type_key(module: &str, name: &str) -> String {
    format!("import(\"{module}\").{name}")
}

/// The named types the file sees from its imports: every declaration each
/// imported module passes on, and each `type` import under its own name.
pub(crate) fn imported_declarations(root: &SyntaxNode, modules: &ImportedModules) -> Vec<NamedDeclaration> {
    let mut declarations = Vec::new();
    let mut seen = HashSet::new();
    for statement in import_statements(root) {
        let Some(interface) = import_reference(&statement).and_then(|reference| modules.get(&reference.path)) else {
            continue;
        };
        for declaration in &interface.declarations {
            if seen.insert(declaration.name.clone()) {
                declarations.push(declaration.clone());
            }
        }
        for import in imports_of(&statement).into_iter().filter(|import| import.is_type) {
            let Some(exported) = &import.exported else { continue };
            let key = interface.types.get(exported.text());
            let declaration =
                key.and_then(|key| interface.declarations.iter().find(|declaration| &declaration.name == key));
            if let Some(declaration) = declaration {
                declarations.push(declaration.named(import.local.text()));
            }
        }
    }
    declarations
}

/// The interface of the module `root`, checked as `module`: `environment`
/// holds its named types, `value_types` its exported values' types.
pub(crate) fn build(
    root: &SyntaxNode,
    module: &str,
    environment: &TypeEnvironment,
    value_types: &HashMap<String, Type>,
    modules: &ImportedModules,
) -> ModuleInterface {
    let own_types = own_type_names(root);
    let renaming: Renaming = own_types.iter().map(|name| (name.clone(), type_key(module, name))).collect();
    let mut declarations: Vec<NamedDeclaration> = own_types
        .iter()
        .filter_map(|name| environment.named_declaration(name))
        .map(|declaration| declaration.renamed(&renaming))
        .collect();
    let mut seen: HashSet<String> = declarations.iter().map(|declaration| declaration.name.clone()).collect();
    for declaration in imported_declarations(root, modules) {
        if seen.insert(declaration.name.clone()) && !own_types.contains(&declaration.name) {
            declarations.push(declaration);
        }
    }
    let mut interface = ModuleInterface { declarations, ..ModuleInterface::default() };
    for name in exported_names(root) {
        let (local, exported) = (name.local.text(), name.exported.text().to_string());
        if let Some(value) = value_types.get(local) {
            interface.values.insert(exported.clone(), value.renamed(&renaming));
        }
        if own_types.contains(local) {
            interface.types.insert(exported, type_key(module, local));
        }
    }
    interface
}

/// The type names the module itself gives: its own type declarations and
/// the names of its `type` imports.
fn own_type_names(root: &SyntaxNode) -> HashSet<String> {
    let declared = crate::type_environment::type_statements(root)
        .filter_map(|statement| crate::syntax_access::declared_name(&statement))
        .map(|name| name.text().to_string());
    let imported = import_statements(root)
        .flat_map(|statement| imports_of(&statement))
        .filter(|import| import.is_type)
        .map(|import| import.local.text().to_string());
    declared.chain(imported).collect()
}
