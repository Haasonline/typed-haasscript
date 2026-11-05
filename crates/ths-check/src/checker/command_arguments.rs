//! A value passed to a TradeServer command: the server converts some
//! values a parameter's type does not name (each conversion verified on the
//! TradeServer), so those are
//! warnings, not errors. A Lua function receives the value itself, so the
//! conversions are for commands only.

use rowan::TextRange;
use ths_syntax::SyntaxNode;

use super::Checker;
use crate::types::{FunctionType, Type};

impl Checker<'_> {
    /// Checks an argument for a parameter of type `accepted` of `function`.
    pub(super) fn check_argument_value(&mut self, function: &FunctionType, argument: &SyntaxNode, accepted: &Type) {
        if !function.is_command {
            self.check_assignable_expression(argument, accepted);
            return;
        }
        let actual = self.check_expression(argument, Some(accepted));
        self.report_argument_value(function, &actual, accepted, argument.text_range());
    }

    /// Reports an argument value of type `actual` that a parameter of type
    /// `accepted` of `function` does not take.
    pub(super) fn report_argument_value(
        &mut self,
        function: &FunctionType,
        actual: &Type,
        accepted: &Type,
        range: TextRange,
    ) {
        // A value that fits but for a possible nil gets the nil warning.
        if !function.is_command || self.is_assignable(&actual.without_nil(), accepted) {
            self.report_if_not_assignable(actual, accepted, range);
        } else if self.converts_to_text(actual, accepted) {
            self.report_text_conversion(actual, accepted, range);
        } else if self.takes_the_first_field(actual, accepted) {
            self.report_first_field(actual, accepted, range);
        } else {
            self.report_if_not_assignable(actual, accepted, range);
        }
    }
}
