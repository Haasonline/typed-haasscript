//! Walks a syntax tree, inferring and checking types. The state and shared
//! helpers live here; each kind of syntax has its own submodule.

mod analysis_recording;
mod calls;
mod command_arguments;
mod command_globals;
mod command_parameters;
mod constant_names;
mod control_flow;
mod expressions;
mod first_field;
mod functions;
mod implicit_any;
mod inferred_targets;
mod known_names;
mod members;
mod module_imports;
mod named_arguments;
mod plain_reassignment;
mod recorded_types;
mod result_containers;
mod series;
mod series_keys;
mod series_variables;
mod statements;
mod tables;
mod text_conversion;
mod unindexable;
mod value_lists;

use std::collections::{HashMap, HashSet};

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::analysis::{FileAnalysis, SymbolKind};
use crate::assignability::Assignability;
use crate::diagnostic::DiagnosticSink;
use crate::module_interface::ImportedModules;
use crate::narrowing::{ConditionNarrowings, narrowings};
use crate::scopes::{NarrowingKey, Scopes};
use crate::syntax_access::{annotation_type, child, declared_name, first_expression, first_name, tokens};
use crate::type_environment::{TypeEnvironment, TypeParameters};
use crate::types::Type;

pub(crate) use recorded_types::RecordedTypes;

use analysis_recording::{AnalysisRecorder, Declared};
use module_imports::ModuleState;

/// The declared type of a variable or field inferred from `initial`: the
/// value's type without nil (nil is always assignable to something
/// inferred, and whether it currently holds nil is tracked by narrowing).
/// Something initialized only with nil can hold anything later.
fn implicit_declared_type(initial: &Type) -> Type {
    match initial.without_nil() {
        Type::Never => Type::Any,
        declared => declared,
    }
}

/// The function whose body is being checked.
struct FunctionContext {
    /// From the return type annotation; `None` means infer it.
    declared_return: Option<Type>,
    /// The values each `return` statement returned.
    returned: Vec<Vec<Type>>,
    /// The type of `...`, if the function takes varargs.
    rest: Option<Type>,
    type_parameters: TypeParameters,
}

/// Which optional rules a check applies.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Rules {
    /// Report implicit `any` (see `implicit_any`).
    pub(crate) strict: bool,
    /// Warn where a read falls back past a `local` that ended (see
    /// `known_names`).
    pub(crate) shadowing: bool,
    /// Report names nothing defines (see `known_names`); only when the
    /// declarations cover every name the server knows.
    pub(crate) unknown_names: bool,
    /// A plain file cannot hold annotations, so what the checker cannot tell
    /// stays `any` instead of being guessed.
    pub(crate) lenient: bool,
}

pub(crate) struct Checker<'environment> {
    environment: &'environment TypeEnvironment,
    assignability: Assignability<'environment>,
    scopes: Scopes,
    sink: DiagnosticSink,
    functions: Vec<FunctionContext>,
    rules: Rules,
    /// Names the script makes known to the server's compile check without
    /// a local in scope (see `known_names`).
    known_names: HashSet<String>,
    recorded: RecordedTypes,
    /// The `defaultValue` arguments of the `DefineParameter` calls being
    /// checked (see `command_parameters`).
    parameter_defaults: RecordedTypes,
    /// In a custom command, the globals reported as created (see
    /// `command_globals`); `None` in any other file.
    command_globals: Option<HashSet<String>>,
    /// The interfaces of the modules the file imports.
    modules: Option<&'environment ImportedModules<'environment>>,
    /// The file's own imports and exports (see `module_imports`).
    module: ModuleState,
    /// What an editor asks of the file, when asked for (see `analysis_recording`).
    analysis: AnalysisRecorder,
}

/// What checking a file gives besides its diagnostics.
pub(crate) struct CheckedRoot {
    pub(crate) sink: DiagnosticSink,
    /// The types of the values the file exports, by local name.
    pub(crate) exported_values: HashMap<String, Type>,
    /// When `with_analysis` asked for it.
    pub(crate) analysis: Option<FileAnalysis>,
}

impl<'environment> Checker<'environment> {
    pub(crate) fn new(environment: &'environment TypeEnvironment, sink: DiagnosticSink, rules: Rules) -> Self {
        Checker {
            environment,
            assignability: Assignability::new(environment),
            scopes: Scopes::new(),
            sink,
            functions: Vec::new(),
            rules,
            known_names: HashSet::new(),
            recorded: RecordedTypes::default(),
            parameter_defaults: RecordedTypes::default(),
            command_globals: None,
            modules: None,
            module: ModuleState::default(),
            analysis: AnalysisRecorder::default(),
        }
    }

    /// Also records what an editor asks of the file (see `FileAnalysis`).
    pub(crate) fn with_analysis(mut self) -> Self {
        self.analysis.enable();
        self
    }

    /// Sees the interfaces of the modules the file imports.
    pub(crate) fn with_modules(mut self, modules: &'environment ImportedModules<'environment>) -> Self {
        self.modules = Some(modules);
        self
    }

    /// Checks `root`, seeing the globals `declaration_roots` declare. Their
    /// own diagnostics are dropped.
    pub(crate) fn check_root(mut self, declaration_roots: &[SyntaxNode], root: &SyntaxNode) -> CheckedRoot {
        self.walk_root(declaration_roots, root);
        let exported_values = self.exported_value_types();
        let analysis = self.finish_analysis();
        CheckedRoot { sink: self.sink, exported_values, analysis }
    }

    /// Checks `root` as `check_root` does, for the types of the
    /// expressions `wanted` names instead of the diagnostics.
    pub(crate) fn record_types(
        mut self,
        declaration_roots: &[SyntaxNode],
        root: &SyntaxNode,
        wanted: RecordedTypes,
    ) -> RecordedTypes {
        self.recorded = wanted;
        self.walk_root(declaration_roots, root);
        self.recorded
    }

    fn walk_root(&mut self, declaration_roots: &[SyntaxNode], root: &SyntaxNode) {
        let reported = std::mem::take(&mut self.sink);
        for declaration_root in declaration_roots {
            self.hoist_declarations(declaration_root, false);
        }
        self.sink = reported;
        self.hoist_declarations(root, true);
        self.known_names = known_names::collect(root);
        self.note_command_file(root);
        self.note_exported_locals(root);
        self.bind_imports(root);
        if let Some(block) = child(root, SyntaxKind::Block) {
            self.check_block(&block);
        }
        self.report_unused_imports(root);
    }

    /// `declare` statements describe globals that exist before the script
    /// runs, so they are visible everywhere, as in TypeScript. Their names
    /// are definitions an editor can go to only in the file being checked.
    fn hoist_declarations(&mut self, root: &SyntaxNode, is_checked_file: bool) {
        for statement in root.descendants() {
            let declared = match statement.kind() {
                SyntaxKind::DeclareFunctionStatement => {
                    let function =
                        self.environment.resolver().function_type(&statement, &TypeParameters::new(), &mut self.sink);
                    let name = child(&statement, SyntaxKind::FunctionName).and_then(|name| first_name(&name));
                    name.map(|name| (name, Type::Function(std::rc::Rc::new(function))))
                }
                SyntaxKind::DeclareVariableStatement => declared_name(&statement).map(|name| {
                    let declared = self.annotation(&statement, SyntaxKind::TypeAnnotation);
                    (name, declared.unwrap_or(Type::Any))
                }),
                _ => None,
            };
            if let Some((name, declared)) = declared {
                let id = self.scopes.declare_global(name.text(), declared);
                let written = is_checked_file.then_some(&name);
                let declared = Declared {
                    kind: SymbolKind::Declared,
                    name: written,
                    visible: None,
                    statement: Some(&statement),
                    import: None,
                };
                self.record_definition(id, name.text(), declared);
            }
        }
    }

    /// The type of a node's annotation child of `annotation_kind`, if any.
    fn annotation(&mut self, node: &SyntaxNode, annotation_kind: SyntaxKind) -> Option<Type> {
        let annotation = annotation_type(node, annotation_kind)?;
        let parameters = self.type_parameters_in_scope();
        Some(self.environment.resolver().resolve(&annotation, &parameters, &mut self.sink))
    }

    /// The type parameters of the functions being checked. A function's own
    /// `<T>` is added by the resolver when its signature is resolved.
    fn type_parameters_in_scope(&self) -> TypeParameters {
        self.functions.last().map(|function| function.type_parameters.clone()).unwrap_or_default()
    }

    fn is_assignable(&self, source: &Type, target: &Type) -> bool {
        self.assignability.is_assignable(source, target)
    }

    /// The type to inspect for members, operators and calls: references
    /// expanded, and a type parameter replaced by its constraint (by
    /// `unknown` when it has none), as TypeScript's apparent type.
    fn expand(&self, value: &Type) -> Type {
        match self.environment.expand(value) {
            Type::Parameter(parameter) => match &parameter.constraint {
                Some(constraint) => self.expand(constraint),
                None => Type::Unknown,
            },
            expanded => expanded,
        }
    }

    /// Checks `expression` against `expected` and reports a mismatch.
    fn check_assignable_expression(&mut self, expression: &SyntaxNode, expected: &Type) -> Type {
        let actual = self.check_expression(expression, Some(expected));
        self.report_if_not_assignable(&actual, expected, expression.text_range());
        actual
    }

    fn report_if_not_assignable(&mut self, actual: &Type, expected: &Type, range: TextRange) {
        if self.is_assignable(actual, expected) {
            return;
        }
        let is_nil_only_in_part = actual.may_be_nil() && *actual != Type::Nil;
        if is_nil_only_in_part && self.is_assignable(&actual.without_nil(), expected) {
            // A possible nil is a warning, as every nil problem is.
            self.sink.warning(
                range,
                format!("Type '{actual}' is not assignable to type '{expected}': the value may be nil."),
            );
            return;
        }
        self.sink.error(range, format!("Type '{actual}' is not assignable to type '{expected}'."));
    }

    /// Reports a value that may be nil where nil cannot be used, and
    /// returns the value's type without nil so checking can continue.
    fn require_not_nil(&mut self, value: &Type, range: TextRange) -> Type {
        if value.may_be_nil() {
            self.report_possibly_nil(range);
            return value.without_nil();
        }
        value.clone()
    }

    /// A value that may be nil is a warning, not an error: it fails only
    /// on the run where it is nil, so it does not fail `ths check`.
    pub(super) fn report_possibly_nil(&mut self, range: TextRange) {
        self.sink.warning(range, "Object is possibly 'nil'.".to_string());
    }

    /// The narrowing key and current type of a reference (`x`, `x.a.b`),
    /// without reporting anything: narrowing asks about expressions that
    /// were already checked.
    pub(super) fn reference(&self, node: &SyntaxNode) -> Option<(NarrowingKey, Type)> {
        match node.kind() {
            SyntaxKind::NameExpression => {
                let id = self.scopes.lookup(first_name(node)?.text())?;
                Some((NarrowingKey::variable(id), self.scopes.current_type(id).clone()))
            }
            SyntaxKind::FieldExpression => {
                let (owner_key, owner_type) = self.reference(&first_expression(node)?)?;
                let field = tokens(node).filter(|token| token.kind() == SyntaxKind::Identifier).last()?;
                let key = owner_key.field(field.text());
                let current = match self.scopes.narrowed(&key) {
                    Some(narrowed) => narrowed.clone(),
                    None => self.quiet_member_type(&owner_type.without_nil(), field.text()),
                };
                Some((key, current))
            }
            _ => None,
        }
    }

    fn quiet_member_type(&self, owner: &Type, name: &str) -> Type {
        match self.expand(owner) {
            Type::Object(object) => match object.properties.get(name) {
                Some(property) if property.optional => Type::union([property.value.clone(), Type::Nil]),
                Some(property) => property.value.clone(),
                None => object.index.as_ref().map_or(Type::Any, |(_, value)| value.clone()),
            },
            _ => Type::Any,
        }
    }

    pub(super) fn condition_narrowings(&self, condition: &SyntaxNode) -> ConditionNarrowings {
        narrowings(condition, &|node| self.reference(node))
    }

    /// Narrowing by assignment, as in TypeScript: of a declared union, keep
    /// the members the assigned value can be. An `any` value says nothing,
    /// so the declared type stays.
    fn narrowed_by_assignment(&self, declared: &Type, assigned: &Type) -> Type {
        if assigned.members().iter().any(Type::is_any) {
            return declared.clone();
        }
        let Type::Union(members) = self.expand(declared) else {
            return declared.clone();
        };
        let kept: Vec<Type> = members
            .iter()
            .filter(|member| assigned.members().iter().any(|part| self.is_assignable(part, member)))
            .cloned()
            .collect();
        if kept.is_empty() { declared.clone() } else { Type::union(kept) }
    }
}
