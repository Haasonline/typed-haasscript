//! What checking a custom command's code tells about its interface, beyond
//! the literal `Define…` arguments (verified on the TradeServer):
//!
//! - What a caller receives: the type the `DefineOutput` or
//!   `DefineOutputIndex` constant allows, narrowed to the type of the value
//!   the command gives when that value is one the constant passes through
//!   unchanged (a `ListNumberType` value arrives as given, an `EnumType`
//!   value as the constant). A value the server would convert, or one no
//!   declaration file can name, leaves the constant's type.
//! - What a caller passes for an `EnumType` parameter: a constant of its
//!   default's category, which the command takes it to be. The server
//!   passes a constant of any category, and the command stops when it
//!   compares it with one of the default's.

use rowan::TextRange;
use ths_syntax::{Dialect, SyntaxNode};

use crate::assignability::Assignability;
use crate::checker::{Checker, RecordedTypes, Rules};
use crate::command_signature::{CommandSignature, ParameterKind};
use crate::declarations::Declarations;
use crate::diagnostic::DiagnosticSink;
use crate::engine_types::{ENUM_TYPE, SERIES_TYPE};
use crate::enum_categories::category_of;
use crate::type_environment::TypeEnvironment;
use crate::types::Type;

pub(crate) struct InterfaceTypes {
    /// What a caller passes for each of the signature's parameters, in its
    /// order, when it is narrower than the parameter's constant allows.
    pub(crate) parameters: Vec<Option<Type>>,
    /// What the call returns; `None` when there is no `DefineOutput`.
    pub(crate) output: Option<Type>,
    /// What each of the signature's output entries holds, in its order.
    pub(crate) entries: Vec<Type>,
}

/// The interface types of the command `root` defines, checking it
/// against `declarations`.
pub(crate) fn infer(
    signature: &CommandSignature,
    root: &SyntaxNode,
    dialect: Dialect,
    declarations: &Declarations,
) -> InterfaceTypes {
    let environment = TypeEnvironment::collect(declarations.roots(), &[], root, &mut DiagnosticSink::default());
    let rules = Rules { lenient: dialect == Dialect::Plain, ..Rules::default() };
    let defaults = signature.parameters.iter().filter_map(|parameter| parameter.default);
    let wanted =
        RecordedTypes::of(signature.output_values.iter().chain(&signature.entry_values).copied().chain(defaults));
    let recorded =
        Checker::new(&environment, DiagnosticSink::default(), rules).record_types(declarations.roots(), root, wanted);
    let narrowing =
        Narrowing { environment: &environment, assignability: Assignability::new(&environment), recorded: &recorded };
    let output = signature.output.map(|kind| {
        let allowed = kind.map_or(Type::Any, constant_type);
        narrowing.narrowed(allowed, &signature.output_values)
    });
    let entries = signature
        .output_entries
        .iter()
        .map(|entry| {
            let allowed = entry.kind.map_or(Type::Any, constant_type);
            let value = entry.index.checked_sub(1).and_then(|position| signature.entry_values.get(position).copied());
            narrowing.narrowed(allowed, value.as_slice())
        })
        .collect();
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| match (parameter.kind, parameter.default) {
            (Some(ParameterKind::Enum), Some(default)) => {
                let category = category_of(&environment, recorded.get(default)?)?;
                narrowing.is_nameable(&category).then_some(category)
            }
            _ => None,
        })
        .collect();
    InterfaceTypes { parameters, output, entries }
}

/// What a caller receives from an output of `kind`, whatever the value.
fn constant_type(kind: ParameterKind) -> Type {
    match kind {
        ParameterKind::Number => Type::Number,
        ParameterKind::ListNumber => {
            Type::union([Type::Array(Box::new(Type::Number)), Type::Opaque(SERIES_TYPE.to_string())])
        }
        ParameterKind::String => Type::String,
        ParameterKind::Boolean => Type::Boolean,
        ParameterKind::Enum => Type::Opaque(ENUM_TYPE.to_string()),
        ParameterKind::Void => Type::Nil,
        ParameterKind::Dynamic | ParameterKind::ListDynamic => Type::Any,
    }
}

struct Narrowing<'environment> {
    environment: &'environment TypeEnvironment,
    assignability: Assignability<'environment>,
    recorded: &'environment RecordedTypes,
}

impl Narrowing<'_> {
    /// The type of the values at `values` when every one was checked, can
    /// be named in a declaration file and is `allowed`; otherwise
    /// `allowed`.
    fn narrowed(&self, allowed: Type, values: &[TextRange]) -> Type {
        if allowed.is_any() || values.is_empty() {
            return allowed;
        }
        let checked: Option<Vec<Type>> = values.iter().map(|range| self.recorded.get(*range).cloned()).collect();
        let Some(checked) = checked else { return allowed };
        let value = Type::union(checked).widened();
        if self.is_nameable(&value) && self.assignability.is_assignable(&value, &allowed) { value } else { allowed }
    }

    /// Whether `value` reads the same in a declaration file, where the
    /// command's own types and inferred table types do not exist.
    fn is_nameable(&self, value: &Type) -> bool {
        match value {
            Type::Nil | Type::Boolean | Type::Number | Type::String => true,
            Type::BooleanLiteral(_) | Type::NumberLiteral(_) | Type::StringLiteral(_) => true,
            Type::Array(element) => self.is_nameable(element),
            Type::Union(members) => members.iter().all(|member| self.is_nameable(member)),
            Type::Reference(name, arguments) => {
                self.environment.is_declaration_file_type(name)
                    && arguments.iter().all(|argument| self.is_nameable(argument))
            }
            Type::Opaque(name) => self.environment.is_declaration_file_type(name),
            _ => false,
        }
    }
}
