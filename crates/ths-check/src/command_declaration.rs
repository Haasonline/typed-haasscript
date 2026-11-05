//! The `declare function CC_<Name>(…)` a custom command's signature gives
//! the scripts that call it, written as Typed HaasScript so it loads like
//! any declaration file. What a caller may pass and receives follows the
//! TradeServer's conversions (verified on the TradeServer):
//! a series or a number for `NumberType`, the constant for `EnumType`,
//! the value given for `ListNumberType`, and a container read by name or
//! index for a `ListDynamicType` output with `DefineOutputIndex` entries.

use std::collections::HashSet;

use ths_syntax::SyntaxKind;

use crate::command_interface_types::InterfaceTypes;
use crate::command_signature::{CommandParameter, CommandSignature, OutputEntry, ParameterKind};
use crate::engine_types::ENUM_TYPE;
use crate::primitive_type_names::{self, ANY, NIL};
use crate::types::Type;

/// What `CC_` adds to a `DefineCommand` name.
pub(crate) const CUSTOM_COMMAND_PREFIX: &str = "CC_";

const NUMBER_ARGUMENT: &str = "number | HaasNumberCollection";
const LIST_NUMBER: &str = "number[] | HaasNumberCollection";

/// The declarations for `signature`, with the types in `interface`, or
/// `None` when its name cannot be a Lua name.
pub(crate) fn render(signature: &CommandSignature, interface: &InterfaceTypes) -> Option<String> {
    if !is_name(&signature.name) {
        return None;
    }
    let command = format!("{CUSTOM_COMMAND_PREFIX}{}", signature.name);
    let mut text = String::new();
    let output = match (signature.output, &interface.output) {
        (Some(Some(ParameterKind::ListDynamic)), _) if !signature.output_entries.is_empty() => {
            let result_name = format!("{command}Result");
            text.push_str(&result_interface(&result_name, &signature.output_entries, &interface.entries));
            result_name
        }
        (_, Some(output)) => output.to_string(),
        (_, None) => NIL.to_string(),
    };
    let parameters = parameter_list(&signature.parameters, &interface.parameters);
    text.push_str(&format!("declare function {command}({parameters}): {output}\n"));
    Some(text)
}

fn parameter_list(parameters: &[CommandParameter], narrowed: &[Option<Type>]) -> String {
    let mut used = HashSet::new();
    parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            let name = match &parameter.name {
                Some(name) if is_name(name) && used.insert(name.clone()) => name.clone(),
                _ => format!("parameter{}", position + 1),
            };
            let optional = if parameter.required { "" } else { "?" };
            let accepted = match narrowed.get(position) {
                Some(Some(narrowed)) => narrowed.to_string(),
                _ => parameter.kind.map_or(ANY, argument_type).to_string(),
            };
            format!("{name}{optional}: {accepted}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A container with each entry under its name and its index.
fn result_interface(interface: &str, entries: &[OutputEntry], values: &[Type]) -> String {
    let mut members = Vec::new();
    for (entry, value) in entries.iter().zip(values) {
        if is_name(&entry.name) {
            members.push(format!("{}: {value}", entry.name));
        }
        members.push(format!("{}: {value}", entry.index));
    }
    members.push(format!("[index: number]: {ANY}"));
    format!("interface {interface} {{ {} }}\n", members.join("; "))
}

/// What a caller may pass for a parameter of `kind`.
fn argument_type(kind: ParameterKind) -> &'static str {
    match kind {
        ParameterKind::Number => NUMBER_ARGUMENT,
        ParameterKind::ListNumber => LIST_NUMBER,
        ParameterKind::String => primitive_type_names::STRING,
        ParameterKind::Boolean => primitive_type_names::BOOLEAN,
        ParameterKind::Enum => ENUM_TYPE,
        ParameterKind::Dynamic | ParameterKind::ListDynamic | ParameterKind::Void => ANY,
    }
}

/// A Lua name that is not a keyword.
fn is_name(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && SyntaxKind::keyword_from_name(text).is_none()
}
