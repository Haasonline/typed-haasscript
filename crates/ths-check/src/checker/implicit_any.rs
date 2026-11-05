//! The `strict` setting, TypeScript's `noImplicitAny`: a parameter or
//! variable whose type is neither annotated nor inferred is reported, since
//! it is an unchecked `any`. Off by default, so code without annotations
//! still checks clean.

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use crate::syntax_access::{annotation_type, child, children, first_name, token};
use crate::types::FunctionType;

impl Checker<'_> {
    /// Reports each parameter of `function` (a function body or a lambda)
    /// that `signature` still has as implicit: no annotation and no type
    /// from the context, as a callback gets. `...` is left alone: contextual
    /// typing does not reach it yet.
    pub(super) fn report_implicit_parameters(&mut self, function: &SyntaxNode, signature: &FunctionType) {
        if !self.rules.strict {
            return;
        }
        let Some(list) = child(function, SyntaxKind::ParameterList) else {
            return;
        };
        for parameter in children(&list, SyntaxKind::Parameter) {
            if token(&parameter, SyntaxKind::TripleDot).is_some() {
                continue;
            }
            let Some(name) = first_name(&parameter) else {
                continue;
            };
            let is_implicit =
                signature.parameters.iter().any(|declared| declared.implicit && declared.name == name.text());
            if is_implicit {
                self.sink
                    .error(name.text_range(), format!("Parameter '{}' implicitly has an 'any' type.", name.text()));
            }
        }
    }

    /// Reports `local x` with neither an annotation nor a value.
    pub(super) fn report_implicit_local(&mut self, binding: &SyntaxNode) {
        if !self.rules.strict || annotation_type(binding, SyntaxKind::TypeAnnotation).is_some() {
            return;
        }
        if let Some(name) = first_name(binding) {
            self.sink.error(name.text_range(), format!("Variable '{}' implicitly has an 'any' type.", name.text()));
        }
    }
}
