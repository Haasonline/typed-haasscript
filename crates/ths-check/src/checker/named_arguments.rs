//! Named-argument tables: a table of `name = value` fields as a call's last
//! argument, in the slot of an optional parameter that takes no table,
//! fills that parameter and later ones by name:
//! `PlaceGoLongOrder(price, amount, {type = MarketOrderType, note = "x"})`.
//! The table may be written in place, held in a variable, or returned by
//! a call that gives only it. Each rule is
//! verified on the TradeServer: a key
//! matching no parameter from the slot on, including one naming an earlier
//! parameter, is ignored beside a key that matches (a warning: the call
//! still works); when no key matches, the server reads the table as the
//! slot's value (an error). A table in a required
//! parameter's slot stops the script.

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use super::calls::accepted_type;
use crate::syntax_access::{first_expression, first_name};
use crate::types::{FunctionType, Parameter, Type};

/// One `name = value` of a named-argument table.
struct NamedArgument {
    name: String,
    /// Where a problem with this argument is reported.
    range: TextRange,
    value: NamedValue,
}

enum NamedValue {
    /// A field written in the table, checked against its parameter.
    Expression(SyntaxNode),
    /// A field of a table held in a variable or returned by a call, already
    /// checked.
    Type(Type),
}

/// The position of the parameter `name` names, if it is the slot's or a
/// later one.
fn matching_position(function: &FunctionType, index: usize, name: &str) -> Option<usize> {
    function.parameters.iter().position(|parameter| parameter.name == name).filter(|position| *position >= index)
}

fn is_named_table(argument: &SyntaxNode) -> bool {
    argument.kind() == SyntaxKind::TableConstructor
        && argument.children().next().is_some()
        && argument.children().all(|field| field.kind() == SyntaxKind::NamedField)
}

fn written_arguments(table: &SyntaxNode) -> Vec<NamedArgument> {
    table
        .children()
        .filter_map(|field| {
            let (name, value) = (first_name(&field)?, first_expression(&field)?);
            Some(NamedArgument {
                name: name.text().to_string(),
                range: name.text_range(),
                value: NamedValue::Expression(value),
            })
        })
        .collect()
}

impl Checker<'_> {
    /// Checks `argument`, the call's argument at `index`, as a table of
    /// named arguments when it is one, and returns whether it was. The
    /// caller checks any other argument as usual.
    pub(super) fn check_named_arguments(
        &mut self,
        function: &FunctionType,
        argument: &SyntaxNode,
        index: usize,
        is_last: bool,
    ) -> bool {
        if !self.takes_named_arguments(function, index, is_last) {
            return false;
        }
        let named = if is_named_table(argument) {
            written_arguments(argument)
        } else {
            let Some((_, held)) = self.reference(argument) else {
                return false;
            };
            let Some(named) = self.named_fields(&held, argument.text_range()) else {
                return false;
            };
            self.check_expression(argument, None);
            named
        };
        self.check_named_fields(function, index, named, argument.text_range());
        true
    }

    /// Checks `value`, the single value a call in the last position gives
    /// for the argument at `index`, as a table of named arguments when it
    /// is one, and returns whether it was. The call is already checked.
    pub(super) fn check_returned_named_arguments(
        &mut self,
        function: &FunctionType,
        value: &Type,
        range: TextRange,
        index: usize,
        is_last: bool,
    ) -> bool {
        if !self.takes_named_arguments(function, index, is_last) {
            return false;
        }
        let Some(named) = self.named_fields(value, range) else {
            return false;
        };
        self.check_named_fields(function, index, named, range);
        true
    }

    /// Whether a table at `index` would be named arguments: it is the last
    /// argument, and its slot's parameter takes no table.
    fn takes_named_arguments(&self, function: &FunctionType, index: usize, is_last: bool) -> bool {
        is_last && function.parameters.get(index).is_some_and(|slot| !self.accepts_tables(&slot.value))
    }

    /// The fields of `held`, a table of named fields, each reported at
    /// `range`; `None` for any other type.
    fn named_fields(&self, held: &Type, range: TextRange) -> Option<Vec<NamedArgument>> {
        let Type::Object(object) = self.expand(&held.without_nil()) else {
            return None;
        };
        // A numbered field is positional, which makes the table a value.
        let has_positional = object.properties.keys().any(|name| name.parse::<f64>().is_ok());
        if object.properties.is_empty() || object.index.is_some() || has_positional {
            return None;
        }
        Some(
            object
                .properties
                .iter()
                .map(|(name, property)| NamedArgument {
                    name: name.clone(),
                    range,
                    value: NamedValue::Type(property.value.clone()),
                })
                .collect(),
        )
    }

    /// Checks the named arguments of a table at `index`, spanning `range`.
    fn check_named_fields(
        &mut self,
        function: &FunctionType,
        index: usize,
        named: Vec<NamedArgument>,
        range: TextRange,
    ) {
        let slot = &function.parameters[index];
        if !slot.optional {
            for named_argument in &named {
                if let NamedValue::Expression(value) = &named_argument.value {
                    self.check_expression(value, None);
                }
            }
            self.sink.error(
                range,
                format!(
                    "A table of named arguments works only in an optional parameter's slot: the server reads this one as '{}' and stops the script.",
                    slot.name
                ),
            );
            return;
        }
        let any_matches = named.iter().any(|named| matching_position(function, index, &named.name).is_some());
        for named_argument in named {
            self.check_named_argument(function, index, slot, named_argument, any_matches);
        }
    }

    fn check_named_argument(
        &mut self,
        function: &FunctionType,
        index: usize,
        slot: &Parameter,
        named: NamedArgument,
        any_matches: bool,
    ) {
        let accepted =
            matching_position(function, index, &named.name).and_then(|position| accepted_type(function, position));
        match (&named.value, accepted) {
            (NamedValue::Expression(value), Some(accepted)) => {
                self.check_argument_value(function, value, &accepted);
            }
            (NamedValue::Type(value), Some(accepted)) => {
                self.report_argument_value(function, value, &accepted, named.range);
            }
            (value, None) => {
                if let NamedValue::Expression(value) = value {
                    self.check_expression(value, None);
                }
                let names_earlier = function.parameters.iter().any(|parameter| parameter.name == named.name);
                let reason = if names_earlier {
                    format!("names a parameter before '{}'", slot.name)
                } else {
                    format!("matches no parameter from '{}' on", slot.name)
                };
                // Beside a key that matches, the server just skips this one,
                // so the call still works: a warning, not an error.
                if any_matches {
                    self.sink.warning(named.range, format!("Named argument '{}' is ignored: it {reason}.", named.name));
                } else {
                    self.sink.error(
                        named.range,
                        format!(
                            "Named argument '{}' {reason}, and no key here matches: the server reads the table as '{}'.",
                            named.name, slot.name
                        ),
                    );
                }
            }
        }
    }

    /// Whether a parameter of type `value` may take a table itself, which
    /// makes a table argument an ordinary value rather than named
    /// arguments.
    fn accepts_tables(&self, value: &Type) -> bool {
        self.expand(&value.without_nil()).members().iter().any(|member| {
            matches!(
                self.expand(member),
                Type::Any | Type::Unknown | Type::Object(_) | Type::Array(_) | Type::Parameter(_)
            )
        })
    }
}
