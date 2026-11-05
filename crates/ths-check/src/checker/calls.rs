//! Calls: checking arguments against the callee's signature, inferring a
//! generic signature's type arguments, and the type of the result.

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use super::value_lists::{ExpressionList, Supplied};
use crate::inference::Inference;
use crate::syntax_access::{call_arguments, child, first_expression, is_type, tokens};
use crate::types::{FunctionType, Substitution, Type};

impl Checker<'_> {
    pub(super) fn check_call(&mut self, call: &SyntaxNode) -> Type {
        let Some(callee) = first_expression(call) else {
            return Type::Any;
        };
        let callee_type = self.check_expression(&callee, None);
        let callee_type = self.require_not_nil(&callee_type, callee.text_range());
        match self.defined_parameter_kind(&callee, call) {
            Some(kind) => self.check_define_parameter(kind, call, |checker| {
                checker.call_result(&callee_type, call, callee.text_range())
            }),
            None => self.call_result(&callee_type, call, callee.text_range()),
        }
    }

    /// `owner:name(arguments)`: `owner` is passed as `self`, which method
    /// signatures leave out.
    /// `owner:name(...)`. The owner is checked even while the name is not
    /// written yet, so an editor can offer the owner's methods.
    pub(super) fn check_method_call(&mut self, call: &SyntaxNode) -> Type {
        let Some(owner) = first_expression(call) else {
            return Type::Any;
        };
        let name = tokens(call).find(|token| token.kind() == SyntaxKind::Identifier);
        let owner_type = self.check_expression(&owner, None);
        if let Some(colon) = tokens(call).find(|token| token.kind() == SyntaxKind::Colon) {
            self.record_member_site(&colon, name.as_ref(), &owner_type);
        }
        let Some(name) = name else {
            return Type::Any;
        };
        let owner_type = self.require_not_nil(&owner_type, owner.text_range());
        let method = self.member_type(&owner_type, name.text(), name.text_range());
        self.record_field(&name, &method);
        let method = self.require_not_nil(&method, name.text_range());
        let method = match child(call, SyntaxKind::TypeInstantiation) {
            Some(instantiation) => self.instantiated(&method, &instantiation),
            None => method,
        };
        self.call_result(&method, call, name.text_range())
    }

    /// `f<<number>>`: the generic function `f` with type arguments.
    pub(super) fn check_instantiation(&mut self, expression: &SyntaxNode) -> Type {
        let (Some(callee), Some(instantiation)) =
            (first_expression(expression), child(expression, SyntaxKind::TypeInstantiation))
        else {
            return Type::Any;
        };
        let callee_type = self.check_expression(&callee, None);
        let callee_type = self.require_not_nil(&callee_type, callee.text_range());
        self.instantiated(&callee_type, &instantiation)
    }

    /// Binds the leading type parameters of `callee` to the arguments in
    /// `instantiation`, as Luau does; the rest are still inferred at the call.
    fn instantiated(&mut self, callee: &Type, instantiation: &SyntaxNode) -> Type {
        let scope = self.type_parameters_in_scope();
        let arguments: Vec<Type> = instantiation
            .children()
            .filter(|argument| is_type(argument.kind()))
            .map(|argument| self.environment.resolver().resolve(&argument, &scope, &mut self.sink))
            .collect();
        let function = match self.expand(callee) {
            Type::Any => return Type::Any,
            Type::Function(function) if !function.type_parameters.is_empty() => function,
            _ => {
                self.sink.error(instantiation.text_range(), format!("Type '{callee}' takes no type arguments."));
                return callee.clone();
            }
        };
        let declared = function.type_parameters.len();
        if arguments.len() > declared {
            self.sink.error(
                instantiation.text_range(),
                format!("Expected {declared} type argument(s), but got {}.", arguments.len()),
            );
        }
        let mut substitution = Substitution::new();
        for (parameter, argument) in function.type_parameters.iter().zip(arguments) {
            if let Some(constraint) = &parameter.constraint {
                let constraint = constraint.substituted(&substitution);
                if !self.is_assignable(&argument, &constraint) {
                    self.sink.error(
                        instantiation.text_range(),
                        format!("Type '{argument}' does not satisfy the constraint '{constraint}'."),
                    );
                }
            }
            substitution.insert(parameter.name.clone(), argument);
        }
        let mut instantiated = function.substituted(&substitution);
        instantiated.type_parameters = function
            .type_parameters
            .iter()
            .filter(|parameter| !substitution.contains_key(&parameter.name))
            .cloned()
            .collect();
        Type::Function(std::rc::Rc::new(instantiated))
    }

    fn call_result(&mut self, callee: &Type, call: &SyntaxNode, callee_range: TextRange) -> Type {
        let arguments = call_arguments(call);
        let expanded = self.expand(callee);
        // A callable opaque type, such as a color (`Red(50)`).
        let opaque_call = match &expanded {
            Type::Opaque(name) => self.environment.opaque_call_signature(name),
            _ => None,
        };
        if let Some(function) = opaque_call {
            return self.check_arguments(&function, arguments, call);
        }
        match expanded {
            Type::Function(function) => self.check_arguments(&function, arguments, call),
            Type::Any | Type::Unknown | Type::Union(_) => {
                for argument in &arguments {
                    self.check_expression(argument, None);
                }
                Type::Any
            }
            other => {
                for argument in &arguments {
                    self.check_expression(argument, None);
                }
                self.sink.error(
                    callee_range,
                    format!("This expression is not callable. Type '{other}' has no call signatures."),
                );
                Type::Any
            }
        }
    }

    /// Checks the arguments against the signature and returns the call's
    /// result type. A generic signature's type arguments are inferred
    /// first. As in TypeScript, function arguments are then checked last,
    /// typed by what the other arguments inferred, so
    /// `map(prices, |price| price * 2)` types `price` from `prices`.
    fn check_arguments(&mut self, function: &FunctionType, arguments: Vec<SyntaxNode>, call: &SyntaxNode) -> Type {
        let mut inference = Inference::new(self.environment, &function.type_parameters);
        let mut pending = Vec::new();
        let order: Vec<usize> = if function.type_parameters.is_empty() {
            (0..arguments.len()).collect()
        } else {
            let (callbacks, others): (Vec<usize>, Vec<usize>) =
                (0..arguments.len()).partition(|&index| is_function_expression(&arguments[index]));
            others.into_iter().chain(callbacks).collect()
        };
        let mut list = ExpressionList::new(arguments);
        for index in order {
            self.check_argument(function, &mut list, index, &mut inference, &mut pending);
        }
        // Values a final call spreads past the last argument.
        let mut index = list.expression_count();
        while !list.is_exhausted(index) {
            self.check_argument(function, &mut list, index, &mut inference, &mut pending);
            index += 1;
        }
        let substitution = inference.substitution(&Type::Unknown);
        self.check_constraints(&inference, &substitution, call);
        for argument in pending {
            self.report_if_not_assignable(
                &argument.value,
                &argument.accepted.substituted(&substitution),
                argument.range,
            );
        }
        self.check_argument_count(function, &list, call);
        function.returns.substituted(&substitution)
    }

    /// Checks what `list` supplies at `index`. A value whose parameter type
    /// mentions a type parameter adds to `inference`, and is reported once
    /// the type arguments are known.
    fn check_argument(
        &mut self,
        function: &FunctionType,
        list: &mut ExpressionList,
        index: usize,
        inference: &mut Inference,
        pending: &mut Vec<PendingArgument>,
    ) {
        let accepted = accepted_type(function, index);
        match (self.supplied(list, index), accepted) {
            (Supplied::Expression(argument), Some(accepted)) if inference.mentions_parameters(&accepted) => {
                let contextual = accepted.substituted(&inference.substitution(&Type::Any));
                let value = self.check_expression(&argument, Some(&contextual));
                inference.infer(&accepted, &value);
                pending.push(PendingArgument { value, accepted, range: argument.text_range() });
            }
            (Supplied::Expression(argument), Some(accepted)) => {
                let is_last = index + 1 == list.expression_count();
                if !self.check_named_arguments(function, &argument, index, is_last) {
                    self.check_argument_value(function, &argument, &accepted);
                }
            }
            (Supplied::Expression(argument), None) => {
                self.check_expression(&argument, None);
            }
            (Supplied::Spread(spread), Some(accepted)) => {
                // Into `...` a rest value is passed on, or not at all; into a
                // parameter of its own an absent one arrives as nil.
                let value = if index >= function.parameters.len() { spread.value.clone() } else { spread.received() };
                if inference.mentions_parameters(&accepted) {
                    inference.infer(&accepted, &value);
                    pending.push(PendingArgument { value, accepted, range: spread.range });
                } else {
                    let is_last = list.value_count() == Some(index + 1);
                    if !self.check_returned_named_arguments(function, &value, spread.range, index, is_last) {
                        self.report_argument_value(function, &value, &accepted, spread.range);
                    }
                }
            }
            (Supplied::Spread(..), None) | (Supplied::Missing, _) => {}
        }
    }

    fn check_constraints(&mut self, inference: &Inference, substitution: &Substitution, call: &SyntaxNode) {
        for parameter in inference.parameters() {
            let (Some(constraint), Some(inferred)) = (&parameter.constraint, substitution.get(&parameter.name)) else {
                continue;
            };
            let constraint = constraint.substituted(substitution);
            if !self.is_assignable(inferred, &constraint) {
                self.sink.error(
                    argument_list_range(call),
                    format!("Type '{inferred}' does not satisfy the constraint '{constraint}'."),
                );
            }
        }
    }

    /// Too few values, counting what a final call spreads (unknown for
    /// `...` or an unchecked call), or too many argument expressions: extra
    /// spread values are dropped, as Lua does.
    fn check_argument_count(&mut self, function: &FunctionType, list: &ExpressionList, call: &SyntaxNode) {
        let supplied = list.value_count();
        let written = list.expression_count();
        let required = function.parameters.iter().filter(|parameter| !parameter.optional).count();
        let too_few = supplied.is_some_and(|count| count < required);
        let too_many = written > function.parameters.len() && function.rest.is_none();
        if too_few || too_many {
            let expected = if required == function.parameters.len() {
                required.to_string()
            } else {
                format!("{required}-{}", function.parameters.len())
            };
            self.sink.error(
                argument_list_range(call),
                format!("Expected {expected} arguments, but got {}.", supplied.unwrap_or(written)),
            );
        }
    }
}

/// An argument checked before the type arguments were known.
struct PendingArgument {
    value: Type,
    /// The parameter type, before substitution.
    accepted: Type,
    range: TextRange,
}

/// The type the argument at `index` must have, if the signature has a
/// parameter (or `...`) for it.
pub(super) fn accepted_type(function: &FunctionType, index: usize) -> Option<Type> {
    match (function.parameters.get(index), &function.rest) {
        (Some(parameter), _) if parameter.optional => Some(Type::union([parameter.value.clone(), Type::Nil])),
        (Some(parameter), _) => Some(parameter.value.clone()),
        (None, rest) => rest.clone(),
    }
}

fn is_function_expression(argument: &SyntaxNode) -> bool {
    matches!(argument.kind(), SyntaxKind::FunctionExpression | SyntaxKind::LambdaExpression)
}

fn argument_list_range(call: &SyntaxNode) -> TextRange {
    child(call, SyntaxKind::ArgumentList).map_or(call.text_range(), |list| list.text_range())
}
