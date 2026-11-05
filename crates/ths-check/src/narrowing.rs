//! What a condition tells about references when it is true and when it is
//! false. A reference is a variable or a field path from one (`x`,
//! `order.price`); the caller supplies how to find a reference's key and
//! current type.
//!
//! Recognized forms: `r`, `not c`, `r ~= nil` (also `!=`), `r == nil` (in
//! either order), `a and b`, `a or b`, and parentheses.

use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::scopes::NarrowingKey;
use crate::syntax_access::{expressions, first_expression, tokens};
use crate::types::Type;

/// A reference's narrowing key and current type, or `None` if the node is
/// not a reference.
pub(crate) type ReferenceLookup<'lookup> = dyn Fn(&SyntaxNode) -> Option<(NarrowingKey, Type)> + 'lookup;

#[derive(Debug, Default, Clone)]
pub(crate) struct ConditionNarrowings {
    pub(crate) when_true: Vec<(NarrowingKey, Type)>,
    pub(crate) when_false: Vec<(NarrowingKey, Type)>,
}

impl ConditionNarrowings {
    fn swapped(self) -> Self {
        ConditionNarrowings { when_true: self.when_false, when_false: self.when_true }
    }
}

pub(crate) fn narrowings(condition: &SyntaxNode, reference: &ReferenceLookup) -> ConditionNarrowings {
    match condition.kind() {
        SyntaxKind::NameExpression | SyntaxKind::FieldExpression => truthiness(condition, reference),
        SyntaxKind::ParenthesizedExpression => {
            first_expression(condition).map(|inner| narrowings(&inner, reference)).unwrap_or_default()
        }
        SyntaxKind::UnaryExpression if operator(condition) == Some(SyntaxKind::NotKeyword) => {
            first_expression(condition).map(|operand| narrowings(&operand, reference).swapped()).unwrap_or_default()
        }
        SyntaxKind::BinaryExpression => binary_narrowings(condition, reference),
        _ => ConditionNarrowings::default(),
    }
}

fn operator(node: &SyntaxNode) -> Option<SyntaxKind> {
    tokens(node).next().map(|token| token.kind())
}

fn truthiness(node: &SyntaxNode, reference: &ReferenceLookup) -> ConditionNarrowings {
    let Some((key, current)) = reference(node) else {
        return ConditionNarrowings::default();
    };
    ConditionNarrowings {
        when_true: vec![(key.clone(), current.truthy_part())],
        when_false: vec![(key, current.falsy_part())],
    }
}

fn binary_narrowings(node: &SyntaxNode, reference: &ReferenceLookup) -> ConditionNarrowings {
    let operands: Vec<SyntaxNode> = expressions(node).collect();
    let [left, right] = operands.as_slice() else {
        return ConditionNarrowings::default();
    };
    match operator(node) {
        Some(SyntaxKind::AndKeyword) => {
            let left = narrowings(left, reference);
            let right = narrowings(right, reference);
            // True only if both are; false tells nothing certain about either.
            ConditionNarrowings { when_true: [left.when_true, right.when_true].concat(), when_false: Vec::new() }
        }
        Some(SyntaxKind::OrKeyword) => {
            let left = narrowings(left, reference);
            let right = narrowings(right, reference);
            ConditionNarrowings { when_true: Vec::new(), when_false: [left.when_false, right.when_false].concat() }
        }
        Some(SyntaxKind::EqualsEquals) => nil_comparison(left, right, reference),
        Some(SyntaxKind::TildeEquals | SyntaxKind::BangEquals) => nil_comparison(left, right, reference).swapped(),
        _ => ConditionNarrowings::default(),
    }
}

/// `r == nil` or `nil == r`.
fn nil_comparison(left: &SyntaxNode, right: &SyntaxNode, reference: &ReferenceLookup) -> ConditionNarrowings {
    let is_nil = |node: &SyntaxNode| {
        node.kind() == SyntaxKind::LiteralExpression
            && tokens(node).next().is_some_and(|token| token.kind() == SyntaxKind::NilKeyword)
    };
    let compared = if is_nil(right) {
        left
    } else if is_nil(left) {
        right
    } else {
        return ConditionNarrowings::default();
    };
    let Some((key, current)) = reference(compared) else {
        return ConditionNarrowings::default();
    };
    ConditionNarrowings { when_true: vec![(key.clone(), Type::Nil)], when_false: vec![(key, current.without_nil())] }
}
