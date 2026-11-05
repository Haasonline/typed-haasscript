//! A variable or a field of an inferred table, inferred as a number, that
//! is later assigned a series, or the other way round: scripts start a
//! total at `0` and assign a math helper's result to it later, and the math
//! helpers return a series of one element even for plain numbers. That
//! series works like a number in arithmetic, comparisons and command
//! arguments, and breaks only as a `for` bound, a table key or an argument
//! to Lua's own functions (verified on the TradeServer). So the assignment
//! widens the variable or field to `number | HaasNumberCollection` instead
//! of being an error, and the places where a series breaks report it.
//!
//! Only the assignment that brings in the other kind widens it, so a
//! counter that only ever holds numbers stays a number.

use ths_syntax::SyntaxNode;

use super::Checker;
use super::series::{is_series, series_type};
use crate::types::Type;

impl Checker<'_> {
    /// Widens the inferred variable or field `target` when `assigned`
    /// brings a series to a number or a number to a series, and returns
    /// its new declared type.
    pub(super) fn widen_for_series(&mut self, target: &SyntaxNode, assigned: &Type) -> Option<Type> {
        self.retype_inferred(target, assigned, |checker, declared, assigned| {
            checker.widened_for_series(declared, assigned)
        })
    }

    /// `number | HaasNumberCollection` when `assigned` brings the other
    /// kind to `declared`, a number or a series.
    fn widened_for_series(&self, declared: &Type, assigned: &Type) -> Option<Type> {
        let declared = self.expand(declared);
        let assigned = assigned.without_nil();
        let brings_other_kind = match declared {
            Type::Number => assigned.members().iter().any(|member| is_series(&self.expand(member))),
            _ if is_series(&declared) => {
                assigned.members().iter().any(|member| self.is_assignable(member, &Type::Number))
            }
            _ => false,
        };
        let widened = Type::union([Type::Number, series_type()]);
        (brings_other_kind && self.is_assignable(&assigned, &widened)).then_some(widened)
    }
}
