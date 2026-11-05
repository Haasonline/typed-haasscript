//! Checking function bodies and inferring their return types.

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::analysis_recording::Declared;
use super::control_flow::block_always_exits;
use super::{Checker, FunctionContext};
use crate::analysis::SymbolKind;
use crate::syntax_access::{annotation_type, child, children, first_name, token};
use crate::types::{FunctionType, Type};
use crate::visibility::{BindingSite, visible_range};

/// The name `self` takes in a method.
const SELF_NAME: &str = "self";

impl Checker<'_> {
    /// The signature as written: annotated parameters and return type, with
    /// `any` where there is no annotation.
    pub(super) fn declared_signature(&mut self, body: &SyntaxNode) -> FunctionType {
        let parameters = self.type_parameters_in_scope();
        self.environment.resolver().function_type(body, &parameters, &mut self.sink)
    }

    /// Checks a `FunctionBody` and returns its signature, with the return
    /// type inferred when it is not annotated. `expected` supplies types for
    /// unannotated parameters (contextual typing, as for a callback);
    /// `self_type` declares `self` for `function a:method()`.
    pub(super) fn check_function_body(
        &mut self,
        body: &SyntaxNode,
        expected: Option<&FunctionType>,
        self_type: Option<Type>,
    ) -> FunctionType {
        let mut signature = self.declared_signature(body);
        if let Some(expected) = expected {
            apply_contextual_parameters(&mut signature, expected);
        }
        self.report_implicit_parameters(body, &signature);
        let declared_return =
            annotation_type(body, SyntaxKind::ReturnTypeAnnotation).is_some().then(|| signature.returns.clone());
        let block = child(body, SyntaxKind::Block);
        let returned = self.check_in_function_scope(&signature, declared_return.clone(), self_type, body);
        let falls_off_the_end = !block.as_ref().is_some_and(block_always_exits);
        signature.returns = match declared_return {
            Some(declared) => {
                if falls_off_the_end {
                    self.check_ending_return(body, &declared);
                }
                declared
            }
            None => inferred_return(returned, falls_off_the_end),
        };
        signature
    }

    /// Reports `body`, a function that can end without a `return`, when its
    /// `declared` return type does not accept the nil it then returns.
    fn check_ending_return(&mut self, body: &SyntaxNode, declared: &Type) {
        if self.is_assignable(&Type::Nil, declared) || declared.is_any() {
            return;
        }
        let range = token(body, SyntaxKind::EndKeyword).map_or(body.text_range(), |end| end.text_range());
        self.sink.error(
            range,
            format!("Function lacks ending return statement and return type '{declared}' does not include 'nil'."),
        );
    }

    /// Checks `block` with the parameters in scope. Narrowings of outer
    /// variables do not carry into the body: it may run later, after they
    /// changed. Returns the values each `return` statement returned.
    fn check_in_function_scope(
        &mut self,
        signature: &FunctionType,
        declared_return: Option<Type>,
        self_type: Option<Type>,
        body: &SyntaxNode,
    ) -> Vec<Vec<Type>> {
        let block = child(body, SyntaxKind::Block);
        let outer_narrowings = self.scopes.narrowings();
        self.scopes.restore_narrowings(Default::default());
        self.scopes.push();
        if let Some(self_type) = self_type {
            let id = self.scopes.declare(SELF_NAME, self_type);
            let declared = Declared {
                kind: SymbolKind::Parameter,
                name: None,
                visible: Some(body.text_range()),
                statement: None,
                import: None,
            };
            self.record_definition(id, SELF_NAME, declared);
        }
        self.declare_parameters(signature, body);
        let mut type_parameters = self.type_parameters_in_scope();
        for parameter in &signature.type_parameters {
            type_parameters.insert(parameter.name.clone(), parameter.clone());
        }
        self.functions.push(FunctionContext {
            declared_return,
            returned: Vec::new(),
            rest: signature.rest.clone(),
            type_parameters,
        });
        if let Some(block) = &block {
            self.check_block(block);
        }
        let context = self.functions.pop().expect("pushed above");
        self.scopes.pop();
        self.scopes.restore_narrowings(outer_narrowings);
        context.returned
    }

    /// `|parameters| expression`: the expression is the return value.
    pub(super) fn check_lambda(&mut self, lambda: &SyntaxNode, expected: Option<&FunctionType>) -> FunctionType {
        let mut signature = self.declared_signature(lambda);
        if let Some(expected) = expected {
            apply_contextual_parameters(&mut signature, expected);
        }
        self.report_implicit_parameters(lambda, &signature);
        let outer_narrowings = self.scopes.narrowings();
        self.scopes.restore_narrowings(Default::default());
        self.scopes.push();
        self.declare_parameters(&signature, lambda);
        let result = lambda
            .children()
            .filter(|node| crate::syntax_access::is_expression(node.kind()))
            .last()
            .map(|body| match expected {
                Some(expected) if !expected.returns.is_any() => {
                    self.check_assignable_expression(&body, &expected.returns)
                }
                _ => self.check_expression(&body, None),
            })
            .unwrap_or(Type::Nil);
        self.scopes.pop();
        self.scopes.restore_narrowings(outer_narrowings);
        signature.returns = result.widened();
        signature
    }

    /// An optional parameter may be nil inside the function; an implicit
    /// one is an inferred `any`. `function` is the `FunctionBody` or lambda
    /// whose parameter list names them.
    fn declare_parameters(&mut self, signature: &FunctionType, function: &SyntaxNode) {
        let written: Vec<_> = child(function, SyntaxKind::ParameterList)
            .map(|list| children(&list, SyntaxKind::Parameter).filter_map(|parameter| first_name(&parameter)).collect())
            .unwrap_or_default();
        for parameter in &signature.parameters {
            let id = if parameter.implicit {
                self.scopes.declare_implicit(&parameter.name, Type::Any)
            } else if parameter.optional {
                self.scopes.declare(&parameter.name, Type::union([parameter.value.clone(), Type::Nil]))
            } else {
                self.scopes.declare(&parameter.name, parameter.value.clone())
            };
            let name = written.iter().find(|name| name.text() == parameter.name);
            let declared = Declared {
                kind: SymbolKind::Parameter,
                name,
                visible: name.and_then(|name| visible_range(name, BindingSite::Parameter)),
                statement: None,
                import: None,
            };
            self.record_definition(id, &parameter.name, declared);
        }
    }

    /// The type of `...` in the function being checked.
    pub(super) fn vararg_type(&self) -> Type {
        self.functions.last().and_then(|function| function.rest.clone()).unwrap_or(Type::Any)
    }
}

/// The return type of a function without an annotation: at each position,
/// the union of what every `return` returned there (nil where one returned
/// fewer values). Falling off the end returns no values.
fn inferred_return(mut returned: Vec<Vec<Type>>, falls_off_the_end: bool) -> Type {
    if falls_off_the_end || returned.is_empty() {
        returned.push(Vec::new());
    }
    let count = returned.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let mut positions: Vec<Type> = (0..count)
        .map(|index| Type::union(returned.iter().map(|values| values.get(index).map_or(Type::Nil, Type::widened))))
        .collect();
    if count == 1 {
        return positions.pop().expect("one position");
    }
    Type::Values(positions, None)
}

/// Contextual typing, as for a callback: an unannotated parameter takes
/// the expected function's parameter type.
fn apply_contextual_parameters(signature: &mut FunctionType, expected: &FunctionType) {
    for (parameter, contextual) in signature.parameters.iter_mut().zip(&expected.parameters) {
        if parameter.implicit {
            parameter.value = contextual.value.clone();
            parameter.optional = contextual.optional;
            parameter.implicit = false;
        }
    }
}
