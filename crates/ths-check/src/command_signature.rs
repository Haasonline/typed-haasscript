//! A custom command's interface, read from the `Define…` calls in its own
//! code: `DefineCommand(name, …)` names it, each `DefineParameter(type,
//! name, description, isRequired, …)` adds a parameter in call order, and
//! `DefineOutput(type, …)` with any `DefineOutputIndex(index, type, name,
//! …)` entries describes what a caller receives (verified on the
//! TradeServer). Only literal arguments
//! are read: what is not a literal is left unknown.

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::literals::literal_type;
use crate::syntax_access::{call_arguments, first_expression, first_name, tokens};
use crate::types::Type;

pub(crate) const DEFINE_COMMAND: &str = "DefineCommand";
pub(crate) const DEFINE_PARAMETER: &str = "DefineParameter";
/// `DefineParameter`'s `defaultValue` argument.
pub(crate) const DEFAULT_VALUE_POSITION: usize = 4;
pub(crate) const DEFINE_OUTPUT: &str = "DefineOutput";
pub(crate) const DEFINE_OUTPUT_INDEX: &str = "DefineOutputIndex";

/// The parameter type constants `DefineParameter` and `DefineOutput` take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParameterKind {
    Number,
    ListNumber,
    String,
    Boolean,
    Enum,
    Dynamic,
    ListDynamic,
    Void,
}

impl ParameterKind {
    fn from_constant(name: &str) -> Option<ParameterKind> {
        let kind = match name {
            "NumberType" => ParameterKind::Number,
            "ListNumberType" => ParameterKind::ListNumber,
            "StringType" => ParameterKind::String,
            "BooleanType" => ParameterKind::Boolean,
            "EnumType" => ParameterKind::Enum,
            "DynamicType" => ParameterKind::Dynamic,
            "ListDynamicType" => ParameterKind::ListDynamic,
            "VoidType" => ParameterKind::Void,
            _ => return None,
        };
        Some(kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandParameter {
    pub(crate) name: Option<String>,
    /// `None` when the type argument is not one of the constants.
    pub(crate) kind: Option<ParameterKind>,
    /// Only a literal `true` makes a parameter required.
    pub(crate) required: bool,
    /// The `defaultValue` argument.
    pub(crate) default: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputEntry {
    pub(crate) index: usize,
    pub(crate) kind: Option<ParameterKind>,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSignature {
    pub(crate) name: String,
    pub(crate) parameters: Vec<CommandParameter>,
    /// `None` when there is no `DefineOutput`; `Some(None)` when its type
    /// argument is not a constant.
    pub(crate) output: Option<Option<ParameterKind>>,
    /// Every `DefineOutputIndex` entry; empty when any one of them is not
    /// written with literals, since the rest would describe it wrongly.
    pub(crate) output_entries: Vec<OutputEntry>,
    /// The value each `DefineOutput` gives.
    pub(crate) output_values: Vec<TextRange>,
    /// The value of each field of the table constructor the one
    /// `DefineOutput` gives, in order: what the entry with index 1, 2, …
    /// holds. Empty for any other output.
    pub(crate) entry_values: Vec<TextRange>,
}

/// The signature of the command `root` defines, or `None` when it calls
/// no `DefineCommand` with a literal name.
pub(crate) fn read(root: &SyntaxNode) -> Option<CommandSignature> {
    let mut name = None;
    let mut parameters = Vec::new();
    let mut output = None;
    let mut output_entries = Vec::new();
    let mut entries_are_literal = true;
    let mut output_value_nodes = Vec::new();
    for call in root.descendants().filter(|node| node.kind() == SyntaxKind::CallExpression) {
        let Some(callee) = callee_name(&call) else { continue };
        let arguments = call_arguments(&call);
        match callee.as_str() {
            DEFINE_COMMAND => name = name.or_else(|| arguments.first().and_then(string_literal)),
            DEFINE_PARAMETER => parameters.push(CommandParameter {
                kind: arguments.first().and_then(constant_kind),
                name: arguments.get(1).and_then(string_literal),
                required: arguments.get(3).is_some_and(|argument| boolean_literal(argument) == Some(true)),
                default: arguments.get(DEFAULT_VALUE_POSITION).map(SyntaxNode::text_range),
            }),
            DEFINE_OUTPUT => {
                output = Some(arguments.first().and_then(constant_kind));
                output_value_nodes.extend(arguments.get(1).cloned());
            }
            DEFINE_OUTPUT_INDEX => match output_entry(&arguments) {
                Some(entry) => output_entries.push(entry),
                None => entries_are_literal = false,
            },
            _ => {}
        }
    }
    if !entries_are_literal {
        output_entries.clear();
    }
    output_entries.sort_by_key(|entry| entry.index);
    let entry_values = match output_value_nodes.as_slice() {
        [only] => field_values(only),
        _ => Vec::new(),
    };
    let output_values = output_value_nodes.iter().map(SyntaxNode::text_range).collect();
    Some(CommandSignature { name: name?, parameters, output, output_entries, output_values, entry_values })
}

/// The values of a table constructor's fields, in order; empty when
/// `value` is no table constructor or has a `[key] = value` field, whose
/// position is not certain.
fn field_values(value: &SyntaxNode) -> Vec<TextRange> {
    if value.kind() != SyntaxKind::TableConstructor {
        return Vec::new();
    }
    let mut values = Vec::new();
    for field in value.children() {
        match field.kind() {
            SyntaxKind::NamedField | SyntaxKind::PositionalField => match first_expression(&field) {
                Some(field_value) => values.push(field_value.text_range()),
                None => return Vec::new(),
            },
            SyntaxKind::IndexedField => return Vec::new(),
            _ => {}
        }
    }
    values
}

fn output_entry(arguments: &[SyntaxNode]) -> Option<OutputEntry> {
    let index = arguments.first().and_then(integer_literal)?;
    let kind = constant_kind(arguments.get(1)?);
    let name = string_literal(arguments.get(2)?)?;
    Some(OutputEntry { index, kind, name })
}

/// `f` in `f(…)`, when the callee is a bare name.
pub(crate) fn callee_name(call: &SyntaxNode) -> Option<String> {
    let callee = first_expression(call)?;
    (callee.kind() == SyntaxKind::NameExpression).then(|| first_name(&callee).map(|name| name.text().to_string()))?
}

pub(crate) fn constant_kind(argument: &SyntaxNode) -> Option<ParameterKind> {
    if argument.kind() != SyntaxKind::NameExpression {
        return None;
    }
    ParameterKind::from_constant(first_name(argument)?.text())
}

fn literal(argument: &SyntaxNode) -> Option<Type> {
    if argument.kind() != SyntaxKind::LiteralExpression {
        return None;
    }
    tokens(argument).next().as_ref().and_then(literal_type)
}

fn string_literal(argument: &SyntaxNode) -> Option<String> {
    match literal(argument)? {
        Type::StringLiteral(value) => Some(value),
        _ => None,
    }
}

fn boolean_literal(argument: &SyntaxNode) -> Option<bool> {
    match literal(argument)? {
        Type::BooleanLiteral(value) => Some(value),
        _ => None,
    }
}

fn integer_literal(argument: &SyntaxNode) -> Option<usize> {
    match literal(argument)? {
        Type::NumberLiteral(value) => value.parse().ok(),
        _ => None,
    }
}
