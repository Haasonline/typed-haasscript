//! Lua's adjustment of expression lists: in `local a, b, c = x, f()` every
//! expression gives one value except the last, which, when it is a call or
//! `...`, spreads its values over the remaining targets.

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use crate::spreading_casts::is_list_cast;
use crate::syntax_access::{child, expressions};
use crate::types::Type;

/// The values a spreading expression produces.
pub(super) struct ValueList {
    values: Vec<Type>,
    /// The type of every value past `values` when their number is not known
    /// (`...`, a call to something unchecked, or a declared `...T` return);
    /// without it they are nil.
    rest: Option<Type>,
    /// Declared rest values may be absent: a target of its own then
    /// receives nil. `...` and unchecked calls are trusted, as before.
    rest_may_be_absent: bool,
}

impl ValueList {
    /// What a call that returned `result` produces.
    fn from_result(result: Type) -> ValueList {
        match result {
            Type::Values(values, rest) => ValueList { values, rest: rest.map(|rest| *rest), rest_may_be_absent: true },
            Type::Any => ValueList { values: vec![Type::Any], rest: Some(Type::Any), rest_may_be_absent: false },
            single => ValueList { values: vec![single], rest: None, rest_may_be_absent: false },
        }
    }

    fn get(&self, index: usize, range: TextRange) -> SpreadValue {
        let (value, may_be_absent) = match (self.values.get(index), &self.rest) {
            (Some(value), _) => (value.clone(), false),
            (None, Some(rest)) => (rest.clone(), self.rest_may_be_absent),
            (None, None) => (Type::Nil, false),
        };
        SpreadValue { value, may_be_absent, range }
    }

    /// How many values, if known.
    fn count(&self) -> Option<usize> {
        self.rest.is_none().then_some(self.values.len())
    }
}

/// What an expression list supplies to the target at one position.
pub(super) enum Supplied {
    /// An expression of its own, for the caller to check against the
    /// target (with contextual typing).
    Expression(SyntaxNode),
    /// A value spread from the last expression, already checked.
    Spread(SpreadValue),
    Missing,
}

pub(super) struct SpreadValue {
    /// The value, when present.
    pub(super) value: Type,
    /// A declared rest value, which the call may not return.
    pub(super) may_be_absent: bool,
    /// The spreading expression's, for reporting.
    pub(super) range: TextRange,
}

impl SpreadValue {
    /// What a target of its own receives: nil when the value is absent.
    pub(super) fn received(&self) -> Type {
        if self.may_be_absent { Type::union([self.value.clone(), Type::Nil]) } else { self.value.clone() }
    }
}

/// The expressions of a list, and what its last one spread, once checked.
pub(super) struct ExpressionList {
    expressions: Vec<SyntaxNode>,
    spread: Option<ValueList>,
}

impl ExpressionList {
    pub(super) fn new(expressions: Vec<SyntaxNode>) -> ExpressionList {
        ExpressionList { expressions, spread: None }
    }

    /// The `ExpressionList` child of `node`, empty if there is none.
    pub(super) fn of(node: &SyntaxNode) -> ExpressionList {
        ExpressionList::new(
            child(node, SyntaxKind::ExpressionList).map(|list| expressions(&list).collect()).unwrap_or_default(),
        )
    }

    pub(super) fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    /// The position of the last expression, when it spreads.
    fn spread_start(&self) -> Option<usize> {
        let last = self.expressions.len().checked_sub(1)?;
        spreads(&self.expressions[last]).then_some(last)
    }

    /// Whether nothing is supplied at `index` or after it. Positions past
    /// the last expression are known once the spread has been checked.
    pub(super) fn is_exhausted(&self, index: usize) -> bool {
        if index < self.expressions.len() {
            return false;
        }
        match (self.spread_start(), &self.spread) {
            (Some(start), Some(spread)) => index - start >= spread.values.len(),
            _ => true,
        }
    }

    /// How many values the list supplies, if known.
    pub(super) fn value_count(&self) -> Option<usize> {
        match (self.spread_start(), &self.spread) {
            (Some(start), Some(spread)) => spread.count().map(|count| start + count),
            (Some(_), None) => None,
            (None, _) => Some(self.expressions.len()),
        }
    }
}

/// A call, `...` or a cast of one to several values, whose values all
/// count when it ends a list.
fn spreads(expression: &SyntaxNode) -> bool {
    match expression.kind() {
        SyntaxKind::CallExpression | SyntaxKind::MethodCallExpression | SyntaxKind::VarargExpression => true,
        SyntaxKind::CastExpression => is_list_cast(expression),
        _ => false,
    }
}

impl Checker<'_> {
    /// What `list` supplies to the target at `index`. The spreading last
    /// expression is checked the first time a position needs it.
    pub(super) fn supplied(&mut self, list: &mut ExpressionList, index: usize) -> Supplied {
        if let Some(start) = list.spread_start()
            && index >= start
        {
            let expression = list.expressions[start].clone();
            if list.spread.is_none() {
                list.spread = Some(self.check_values(&expression));
            }
            let spread = list.spread.as_ref().expect("checked above");
            return Supplied::Spread(spread.get(index - start, expression.text_range()));
        }
        match list.expressions.get(index) {
            Some(expression) => Supplied::Expression(expression.clone()),
            None => Supplied::Missing,
        }
    }

    /// Reports a final call that returns fewer values than the targets
    /// left for it, as Luau does: the targets past them are always nil.
    /// Only a known, fixed count is checked; rest values, unchecked calls
    /// and `...` may supply any number.
    pub(super) fn check_enough_values(&mut self, list: &ExpressionList, targets: usize) {
        let (Some(start), Some(spread)) = (list.spread_start(), &list.spread) else {
            return;
        };
        let (Some(returned), Some(required)) = (spread.count(), targets.checked_sub(start)) else {
            return;
        };
        if returned < required {
            self.sink.error(
                list.expressions[start].text_range(),
                format!("The call returns {returned} value(s), but {required} are required here."),
            );
        }
    }

    /// Checks the expressions past the first `taken`, which no target took.
    pub(super) fn check_untaken(&mut self, list: &ExpressionList, taken: usize) {
        for extra in list.expressions.iter().skip(taken) {
            self.check_expression(extra, None);
        }
    }

    fn check_values(&mut self, expression: &SyntaxNode) -> ValueList {
        let result = match expression.kind() {
            SyntaxKind::CallExpression => self.check_call(expression),
            SyntaxKind::MethodCallExpression => self.check_method_call(expression),
            SyntaxKind::CastExpression => self.check_cast(expression),
            _ => {
                return ValueList { values: Vec::new(), rest: Some(self.vararg_type()), rest_may_be_absent: false };
            }
        };
        self.note_type(expression, &result.value_at(0));
        ValueList::from_result(result)
    }
}
