//! Named types (`type` aliases and `interface`s) and the conversion of type
//! syntax into [`Type`]s.
//!
//! Declarations are hoisted, as in TypeScript: every name is registered
//! before any body is resolved, and names inside bodies stay
//! [`Type::Reference`]s, so recursive declarations stay finite. Type
//! arguments are substituted, and an interface's `extends` list merged, when
//! a reference is expanded, so an interface may extend one declared later in
//! the file.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::diagnostic::DiagnosticSink;
use crate::literals::{literal_type, normalized_number};
use crate::primitive_type_names;
use crate::syntax_access::{
    annotation_type, child, children, declared_name, first_name, first_type, is_type, token, tokens,
};
use crate::types::{FunctionType, ObjectType, Parameter, Property, Renaming, Substitution, Type, TypeParameter};

/// Type parameters in scope, by name.
pub(crate) type TypeParameters = HashMap<String, Rc<TypeParameter>>;

const VARIADIC_PLACEMENT_MESSAGE: &str = "A variadic type is only allowed at the end of a return type.";

/// The `extends` in `T extends C` and `interface A extends B`.
const EXTENDS_KEYWORD: &str = "extends";

fn primitive(name: &str) -> Option<Type> {
    let primitive = match name {
        primitive_type_names::NUMBER => Type::Number,
        primitive_type_names::STRING => Type::String,
        primitive_type_names::BOOLEAN => Type::Boolean,
        primitive_type_names::ANY => Type::Any,
        primitive_type_names::UNKNOWN => Type::Unknown,
        primitive_type_names::NEVER => Type::Never,
        _ => return None,
    };
    Some(primitive)
}

/// How many expansion steps `expand` takes before giving up, which only
/// happens for a cycle such as `type A = B; type B = A`.
const EXPANSION_LIMIT: usize = 64;

/// How many type arguments a declared name takes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Arity {
    /// Type parameters without a default.
    required: usize,
    total: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    parameters: Vec<Rc<TypeParameter>>,
    body: Type,
    /// For interfaces: the `extends` list, merged under `body` on expansion.
    /// For opaque types: the bases.
    extends: Vec<Type>,
    /// For opaque types: what calling a value of the type does.
    call: Option<Rc<FunctionType>>,
}

impl Declaration {
    fn renamed(&self, renaming: &Renaming) -> Declaration {
        Declaration {
            parameters: self.parameters.iter().map(|parameter| Rc::new(parameter.renamed(renaming))).collect(),
            body: self.body.renamed(renaming),
            extends: self.extends.iter().map(|base| base.renamed(renaming)).collect(),
            call: self.call.as_ref().map(|call| Rc::new(call.renamed(renaming))),
        }
    }
}

/// A named type declared in another file, under the name the checked
/// file knows it by.
#[derive(Debug, Clone)]
pub(crate) struct NamedDeclaration {
    pub(crate) name: String,
    arity: Arity,
    declaration: Declaration,
}

impl NamedDeclaration {
    /// The same declaration under another name.
    pub(crate) fn named(&self, name: &str) -> NamedDeclaration {
        NamedDeclaration { name: name.to_string(), ..self.clone() }
    }

    /// The declaration with the named types `renaming` lists renamed, its
    /// own name too.
    pub(crate) fn renamed(&self, renaming: &Renaming) -> NamedDeclaration {
        NamedDeclaration {
            name: renaming.get(&self.name).cloned().unwrap_or_else(|| self.name.clone()),
            arity: self.arity,
            declaration: self.declaration.renamed(renaming),
        }
    }
}

const OPAQUE_MEMBER_MESSAGE: &str = "A declared type may only have call signatures.";
const CALL_SIGNATURE_PLACEMENT_MESSAGE: &str = "Call signatures are only supported in 'declare type'.";
const CALL_OVERLOAD_MESSAGE: &str = "A declared type may have only one call signature.";

#[derive(Debug, Default)]
pub(crate) struct TypeEnvironment {
    names: HashMap<String, Arity>,
    declarations: HashMap<String, Declaration>,
    /// Interfaces of the declaration files: every one is a container a
    /// command returns.
    containers: HashSet<String>,
    /// Every type the declaration files declare, which a declaration
    /// written for another file can name.
    declaration_file_types: HashSet<String>,
}

impl TypeEnvironment {
    /// Collects every `type`, `interface` and `declare type` statement in
    /// the declaration files and then the file. Only the file's
    /// diagnostics go to `sink`.
    pub(crate) fn collect(
        declaration_roots: &[SyntaxNode],
        imported: &[NamedDeclaration],
        root: &SyntaxNode,
        sink: &mut DiagnosticSink,
    ) -> TypeEnvironment {
        let statements: Vec<(SyntaxNode, bool)> = declaration_roots
            .iter()
            .map(|declaration_root| (declaration_root, false))
            .chain([(root, true)])
            .flat_map(|(root, is_reported)| type_statements(root).map(move |statement| (statement, is_reported)))
            .collect();
        let mut discarded = DiagnosticSink::default();
        let mut environment = TypeEnvironment::default();
        for named in imported {
            environment.names.insert(named.name.clone(), named.arity);
        }
        for (statement, is_reported) in &statements {
            let Some(name) = declared_name(statement) else {
                continue;
            };
            let arity = declared_arity(statement);
            if environment.names.insert(name.text().to_string(), arity).is_some() {
                let sink = if *is_reported { &mut *sink } else { &mut discarded };
                sink.error(name.text_range(), format!("Duplicate identifier '{}'.", name.text()));
            }
        }
        let mut declarations: HashMap<String, Declaration> =
            imported.iter().map(|named| (named.name.clone(), named.declaration.clone())).collect();
        for (statement, is_reported) in &statements {
            let Some(name) = declared_name(statement) else {
                continue;
            };
            let sink = if *is_reported { &mut *sink } else { &mut discarded };
            let declaration = environment.resolver().declaration(statement, sink);
            declarations.insert(name.text().to_string(), declaration);
            if !is_reported {
                environment.declaration_file_types.insert(name.text().to_string());
            }
            if !is_reported && statement.kind() == SyntaxKind::InterfaceStatement {
                environment.containers.insert(name.text().to_string());
            }
        }
        environment.declarations = declarations;
        environment
    }

    /// The named type `name`, as a module's interface passes it on.
    pub(crate) fn named_declaration(&self, name: &str) -> Option<NamedDeclaration> {
        Some(NamedDeclaration {
            name: name.to_string(),
            arity: *self.names.get(name)?,
            declaration: self.declarations.get(name)?.clone(),
        })
    }

    /// Whether a declaration file declares the type `name`.
    pub(crate) fn is_declaration_file_type(&self, name: &str) -> bool {
        self.declaration_file_types.contains(name)
    }

    /// Whether `name` is a container a command returns.
    pub(crate) fn is_container(&self, name: &str) -> bool {
        self.containers.contains(name)
    }

    pub(crate) fn resolver(&self) -> TypeResolver<'_> {
        TypeResolver { names: &self.names }
    }

    /// Whether the opaque type `name` is `ancestor`, or extends it directly
    /// or through its bases.
    pub(crate) fn opaque_extends(&self, name: &str, ancestor: &str) -> bool {
        self.opaque_extends_within(name, ancestor, EXPANSION_LIMIT)
    }

    /// The call signature of the opaque type `name`, its own or the nearest
    /// base's.
    pub(crate) fn opaque_call_signature(&self, name: &str) -> Option<Rc<FunctionType>> {
        self.opaque_call_signature_within(name, EXPANSION_LIMIT)
    }

    fn opaque_call_signature_within(&self, name: &str, budget: usize) -> Option<Rc<FunctionType>> {
        let declaration = self.declarations.get(name)?;
        if let Some(call) = &declaration.call {
            return Some(call.clone());
        }
        let budget = budget.checked_sub(1)?;
        declaration.extends.iter().find_map(|base| match self.expand(base) {
            Type::Opaque(base) => self.opaque_call_signature_within(&base, budget),
            _ => None,
        })
    }

    fn opaque_extends_within(&self, name: &str, ancestor: &str, budget: usize) -> bool {
        if name == ancestor {
            return true;
        }
        let (Some(declaration), Some(budget)) = (self.declarations.get(name), budget.checked_sub(1)) else {
            return false;
        };
        declaration.extends.iter().any(|base| match self.expand(base) {
            Type::Opaque(base) => self.opaque_extends_within(&base, ancestor, budget),
            _ => false,
        })
    }

    /// Follows references until a type that is not one, substituting type
    /// arguments and merging interface bases into the result.
    pub(crate) fn expand(&self, value: &Type) -> Type {
        self.expand_within(value, EXPANSION_LIMIT)
    }

    fn expand_within(&self, value: &Type, budget: usize) -> Type {
        let Type::Reference(name, arguments) = value else {
            return value.clone();
        };
        let (Some(declaration), Some(budget)) = (self.declarations.get(name), budget.checked_sub(1)) else {
            return Type::Any;
        };
        let substitution = bind_arguments(&declaration.parameters, arguments);
        let body = self.expand_within(&declaration.body.substituted(&substitution), budget);
        if declaration.extends.is_empty() {
            return body;
        }
        let Type::Object(own) = &body else {
            return body;
        };
        let mut merged = ObjectType::default();
        for base in &declaration.extends {
            if let Type::Object(base) = self.expand_within(&base.substituted(&substitution), budget) {
                merged.properties.extend(base.properties.clone());
                if base.index.is_some() {
                    merged.index = base.index.clone();
                }
            }
        }
        merged.properties.extend(own.properties.clone());
        if own.index.is_some() {
            merged.index = own.index.clone();
        }
        Type::Object(Rc::new(merged))
    }
}

pub(crate) fn type_statements(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::TypeAliasStatement | SyntaxKind::InterfaceStatement | SyntaxKind::DeclareTypeStatement
        )
    })
}

/// A property signature's name; a numeric one (`1: T`) is spelled as the
/// number literal type is, so `x[1]` finds it.
fn property_name(member: &SyntaxNode) -> Option<String> {
    match first_name(member) {
        Some(name) => Some(name.text().to_string()),
        None => token(member, SyntaxKind::Number).map(|number| normalized_number(number.text())),
    }
}

/// Pairs type parameters with arguments; a missing argument takes the
/// parameter's default, or `any`.
fn bind_arguments(parameters: &[Rc<TypeParameter>], arguments: &[Type]) -> Substitution {
    let mut substitution = Substitution::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let argument = match arguments.get(index) {
            Some(argument) => argument.clone(),
            None => parameter.default.as_ref().map_or(Type::Any, |default| default.substituted(&substitution)),
        };
        substitution.insert(parameter.name.clone(), argument);
    }
    substitution
}

/// Counted from syntax, before any type is resolved, so references can be
/// checked while the declarations themselves are being resolved.
fn declared_arity(statement: &SyntaxNode) -> Arity {
    let parameters: Vec<SyntaxNode> = child(statement, SyntaxKind::TypeParameterList)
        .map(|list| children(&list, SyntaxKind::TypeParameter).collect())
        .unwrap_or_default();
    let defaulted = parameters.iter().filter(|parameter| token(parameter, SyntaxKind::Equals).is_some()).count();
    Arity { required: parameters.len() - defaulted, total: parameters.len() }
}

/// Converts type syntax into [`Type`]s, given the declared type names.
pub(crate) struct TypeResolver<'names> {
    names: &'names HashMap<String, Arity>,
}

impl TypeResolver<'_> {
    fn declaration(&self, statement: &SyntaxNode, sink: &mut DiagnosticSink) -> Declaration {
        let (parameters, scope) = self.type_parameter_list(statement, &TypeParameters::new(), sink);
        if statement.kind() == SyntaxKind::DeclareTypeStatement {
            let name = declared_name(statement).map_or_else(String::new, |name| name.text().to_string());
            return Declaration {
                parameters,
                body: Type::Opaque(name),
                extends: self.extends_clause(statement, &scope, sink),
                call: self.declared_call_signature(statement, &scope, sink),
            };
        }
        if statement.kind() == SyntaxKind::TypeAliasStatement {
            let body = match first_type(statement) {
                Some(body) => self.resolve(&body, &scope, sink),
                None => Type::Any,
            };
            return Declaration { parameters, body, extends: Vec::new(), call: None };
        }
        let extends = self.extends_clause(statement, &scope, sink);
        let body = match child(statement, SyntaxKind::ObjectType) {
            Some(object) => self.resolve(&object, &scope, sink),
            None => Type::Any,
        };
        Declaration { parameters, body, extends, call: None }
    }

    /// The call signature in a `declare type` body, if any. Anything else in
    /// the body is reported: an opaque type has no members.
    fn declared_call_signature(
        &self,
        statement: &SyntaxNode,
        scope: &TypeParameters,
        sink: &mut DiagnosticSink,
    ) -> Option<Rc<FunctionType>> {
        let body = child(statement, SyntaxKind::ObjectType)?;
        let mut call = None;
        for member in body.children() {
            match member.kind() {
                SyntaxKind::CallSignature if call.is_none() => {
                    call = Some(Rc::new(self.function_type(&member, scope, sink)));
                }
                SyntaxKind::CallSignature => sink.error(member.text_range(), CALL_OVERLOAD_MESSAGE.to_string()),
                _ => sink.error(member.text_range(), OPAQUE_MEMBER_MESSAGE.to_string()),
            }
        }
        call
    }

    /// The types in the statement's `extends A, B`, if any.
    fn extends_clause(&self, statement: &SyntaxNode, scope: &TypeParameters, sink: &mut DiagnosticSink) -> Vec<Type> {
        child(statement, SyntaxKind::ExtendsClause)
            .map(|clause| clause.children().map(|base| self.resolve(&base, scope, sink)).collect())
            .unwrap_or_default()
    }

    /// The node's own `<T extends C = D, ...>`, if any, and the type
    /// parameters in scope inside it. Each parameter's constraint and
    /// default may refer to the parameters before it.
    pub(crate) fn type_parameter_list(
        &self,
        node: &SyntaxNode,
        outer: &TypeParameters,
        sink: &mut DiagnosticSink,
    ) -> (Vec<Rc<TypeParameter>>, TypeParameters) {
        let mut scope = outer.clone();
        let mut declared = Vec::new();
        let Some(list) = child(node, SyntaxKind::TypeParameterList) else {
            return (declared, scope);
        };
        for parameter in children(&list, SyntaxKind::TypeParameter) {
            let Some(name) = first_name(&parameter) else {
                continue;
            };
            let types: Vec<SyntaxNode> = parameter.children().filter(|node| is_type(node.kind())).collect();
            let has_constraint = tokens(&parameter).any(|token| token.text() == EXTENDS_KEYWORD);
            let has_default = token(&parameter, SyntaxKind::Equals).is_some();
            let constraint = has_constraint
                .then(|| types.first())
                .flatten()
                .map(|constraint| self.resolve(constraint, &scope, sink));
            let default =
                has_default.then(|| types.last()).flatten().map(|default| self.resolve(default, &scope, sink));
            let type_parameter = Rc::new(TypeParameter { name: name.text().to_string(), constraint, default });
            scope.insert(type_parameter.name.clone(), type_parameter.clone());
            declared.push(type_parameter);
        }
        (declared, scope)
    }

    /// The type a type annotation's syntax stands for.
    pub(crate) fn resolve(&self, node: &SyntaxNode, parameters: &TypeParameters, sink: &mut DiagnosticSink) -> Type {
        match node.kind() {
            SyntaxKind::NameType => self.name_type(node, parameters, sink),
            SyntaxKind::LiteralType => tokens(node).next().and_then(|token| literal_type(&token)).unwrap_or(Type::Any),
            SyntaxKind::ArrayType => {
                let element = self.resolve_first(node, parameters, sink);
                Type::Array(Box::new(element))
            }
            SyntaxKind::UnionType => {
                Type::union(node.children().map(|member| self.resolve(&member, parameters, sink)).collect::<Vec<_>>())
            }
            SyntaxKind::IntersectionType => self.intersection(node, parameters, sink),
            SyntaxKind::ParenthesizedType => self.resolve_first(node, parameters, sink),
            SyntaxKind::FunctionType => Type::Function(Rc::new(self.function_type(node, parameters, sink))),
            SyntaxKind::ObjectType => Type::Object(Rc::new(self.object_type(node, parameters, sink))),
            SyntaxKind::TypeList => {
                sink.error(node.text_range(), "A list of types is only allowed as a return type.".to_string());
                Type::Any
            }
            SyntaxKind::VariadicType => {
                sink.error(node.text_range(), VARIADIC_PLACEMENT_MESSAGE.to_string());
                Type::Any
            }
            _ => Type::Any,
        }
    }

    fn resolve_first(&self, node: &SyntaxNode, parameters: &TypeParameters, sink: &mut DiagnosticSink) -> Type {
        match first_type(node) {
            Some(inner) => self.resolve(&inner, parameters, sink),
            None => Type::Any,
        }
    }

    fn name_type(&self, node: &SyntaxNode, parameters: &TypeParameters, sink: &mut DiagnosticSink) -> Type {
        let Some(name) = first_name(node) else {
            return Type::Any;
        };
        let text = name.text();
        let arguments: Vec<Type> = child(node, SyntaxKind::TypeArgumentList)
            .map(|list| {
                list.children()
                    .filter(|argument| is_type(argument.kind()))
                    .map(|argument| self.resolve(&argument, parameters, sink))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(parameter) = parameters.get(text) {
            return Type::Parameter(parameter.clone());
        }
        if let Some(primitive) = primitive(text) {
            return primitive;
        }
        let Some(arity) = self.names.get(text) else {
            sink.error(name.text_range(), format!("Cannot find name '{text}'."));
            return Type::Any;
        };
        if arguments.len() < arity.required || arguments.len() > arity.total {
            let expected = if arity.required == arity.total {
                arity.total.to_string()
            } else {
                format!("between {} and {}", arity.required, arity.total)
            };
            sink.error(node.text_range(), format!("Generic type '{text}' requires {expected} type argument(s)."));
        }
        Type::Reference(text.to_string(), arguments)
    }

    /// `A & B`: object types merge; anything else is not modelled yet.
    fn intersection(&self, node: &SyntaxNode, parameters: &TypeParameters, sink: &mut DiagnosticSink) -> Type {
        let mut merged = ObjectType::default();
        for member in node.children() {
            match self.resolve(&member, parameters, sink) {
                Type::Object(object) => merged.properties.extend(object.properties.clone()),
                _ => return Type::Any,
            }
        }
        Type::Object(Rc::new(merged))
    }

    fn object_type(&self, node: &SyntaxNode, parameters: &TypeParameters, sink: &mut DiagnosticSink) -> ObjectType {
        let mut object = ObjectType::default();
        for member in node.children() {
            match member.kind() {
                SyntaxKind::PropertySignature => {
                    let Some(name) = property_name(&member) else {
                        continue;
                    };
                    let value = match annotation_type(&member, SyntaxKind::TypeAnnotation) {
                        Some(annotation) => self.resolve(&annotation, parameters, sink),
                        None => Type::Any,
                    };
                    let optional = token(&member, SyntaxKind::Question).is_some();
                    object.properties.insert(name, Property { value, optional });
                }
                SyntaxKind::MethodSignature => {
                    let Some(name) = first_name(&member) else {
                        continue;
                    };
                    let function = self.function_type(&member, parameters, sink);
                    let optional = token(&member, SyntaxKind::Question).is_some();
                    object.properties.insert(
                        name.text().to_string(),
                        Property { value: Type::Function(Rc::new(function)), optional },
                    );
                }
                SyntaxKind::IndexSignature => {
                    let mut annotations =
                        children(&member, SyntaxKind::TypeAnnotation).filter_map(|annotation| first_type(&annotation));
                    let key = annotations.next().map(|key| self.resolve(&key, parameters, sink));
                    let value = annotations.next().map(|value| self.resolve(&value, parameters, sink));
                    object.index = Some((key.unwrap_or(Type::Any), value.unwrap_or(Type::Any)));
                }
                SyntaxKind::CallSignature => {
                    sink.error(member.text_range(), CALL_SIGNATURE_PLACEMENT_MESSAGE.to_string());
                }
                _ => {}
            }
        }
        object
    }

    /// The signature of a function type, method signature, declared
    /// function or function body: its type parameters, `ParameterList` and
    /// return type.
    pub(crate) fn function_type(
        &self,
        node: &SyntaxNode,
        outer: &TypeParameters,
        sink: &mut DiagnosticSink,
    ) -> FunctionType {
        let (type_parameters, scope) = self.type_parameter_list(node, outer, sink);
        let (parameters, rest) = match child(node, SyntaxKind::ParameterList) {
            Some(list) => self.parameters(&list, &scope, sink),
            None => (Vec::new(), None),
        };
        let returns = if node.kind() == SyntaxKind::FunctionType {
            // The return type of `(a: T) => R` is the type after `=>`.
            node.children()
                .filter(|child| is_type(child.kind()))
                .last()
                .map(|returns| self.return_type(&returns, &scope, sink))
                .unwrap_or(Type::Any)
        } else {
            match annotation_type(node, SyntaxKind::ReturnTypeAnnotation) {
                Some(returns) => self.return_type(&returns, &scope, sink),
                None => Type::Any,
            }
        };
        FunctionType {
            type_parameters,
            parameters,
            rest,
            returns,
            is_command: node.kind() == SyntaxKind::DeclareFunctionStatement,
        }
    }

    /// A return type or a cast's type, where `(A, B)` lists several values
    /// and a final `...C` any number more.
    pub(crate) fn return_type(
        &self,
        node: &SyntaxNode,
        parameters: &TypeParameters,
        sink: &mut DiagnosticSink,
    ) -> Type {
        let elements: Vec<SyntaxNode> = match node.kind() {
            SyntaxKind::TypeList => node.children().filter(|value| is_type(value.kind())).collect(),
            SyntaxKind::VariadicType => vec![node.clone()],
            _ => return self.resolve(node, parameters, sink),
        };
        let mut values = Vec::new();
        let mut rest = None;
        for (index, element) in elements.iter().enumerate() {
            let is_last = index + 1 == elements.len();
            match element.kind() {
                SyntaxKind::VariadicType if is_last => {
                    rest = Some(Box::new(self.resolve_first(element, parameters, sink)));
                }
                _ => values.push(self.resolve(element, parameters, sink)),
            }
        }
        Type::Values(values, rest)
    }

    /// Parameters of a `ParameterList`. An unannotated one is an optional
    /// `any`: Lua passes nil for a missing argument, and code without
    /// annotations relies on that.
    pub(crate) fn parameters(
        &self,
        list: &SyntaxNode,
        type_parameters: &TypeParameters,
        sink: &mut DiagnosticSink,
    ) -> (Vec<Parameter>, Option<Type>) {
        let mut parameters = Vec::new();
        let mut rest = None;
        for parameter in children(list, SyntaxKind::Parameter) {
            let annotation = annotation_type(&parameter, SyntaxKind::TypeAnnotation);
            let is_annotated = annotation.is_some();
            let value = match annotation {
                Some(annotation) => self.resolve(&annotation, type_parameters, sink),
                None => Type::Any,
            };
            if token(&parameter, SyntaxKind::TripleDot).is_some() {
                rest = Some(value);
                continue;
            }
            let Some(name) = first_name(&parameter) else {
                continue;
            };
            parameters.push(Parameter {
                name: name.text().to_string(),
                value,
                optional: !is_annotated || token(&parameter, SyntaxKind::Question).is_some(),
                implicit: !is_annotated,
            });
        }
        (parameters, rest)
    }
}
