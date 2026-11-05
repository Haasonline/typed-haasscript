//! A TradeServer command converts any value passed for a string parameter
//! to text, so a non-string there is a warning, not an error (verified on
//! the TradeServer): a number becomes
//! its text, a small one in exponent notation, an enumeration constant its
//! name, a color its `rgba(...)` text, a boolean its text.

use rowan::TextRange;

use super::Checker;
use crate::types::Type;

impl Checker<'_> {
    /// Whether a command converts `actual` to text for a parameter of type
    /// `accepted`. Nil is not converted: it is the missing argument.
    pub(super) fn converts_to_text(&self, actual: &Type, accepted: &Type) -> bool {
        self.is_assignable(&Type::String, accepted) && *actual != Type::Nil
    }

    pub(super) fn report_text_conversion(&mut self, actual: &Type, accepted: &Type, range: TextRange) {
        self.sink.warning(
            range,
            format!(
                "Type '{actual}' is not assignable to type '{accepted}': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
            ),
        );
    }
}
