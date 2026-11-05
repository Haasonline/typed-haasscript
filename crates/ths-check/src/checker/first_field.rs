//! A TradeServer command given a container for a number parameter uses the
//! container's first field, with no error (verified on the TradeServer for
//! `CurrentPrice`, `MACD`, `BBANDS` and `Balance`), so that is a warning,
//! not an error. A container whose first field is not a number or a series
//! stops the script, so passing it stays an error.

use rowan::TextRange;

use super::Checker;
use crate::engine_types::CURRENT_PRICE_TYPE;
use crate::types::Type;

/// A container's first field is its numbered member `1`.
const FIRST_FIELD: &str = "1";

impl Checker<'_> {
    /// Whether a command takes the first field of `actual`, a container,
    /// for a parameter of type `accepted`: the field must fit it.
    pub(super) fn takes_the_first_field(&self, actual: &Type, accepted: &Type) -> bool {
        if !self.is_container_type(actual) {
            return false;
        }
        let Type::Object(container) = self.expand(actual) else {
            return false;
        };
        container.properties.get(FIRST_FIELD).is_some_and(|field| {
            self.is_assignable(&Type::Number, accepted) && self.is_assignable(&field.value, accepted)
        })
    }

    /// `CurrentPrice()`'s first field is the open price, rarely what a
    /// script means, so its warning names the fields to pass instead.
    pub(super) fn report_first_field(&mut self, actual: &Type, accepted: &Type, range: TextRange) {
        let is_current_price = matches!(actual, Type::Reference(name, _) if name == CURRENT_PRICE_TYPE);
        let used = if is_current_price {
            "its first field, the open price; pass '.close', '.bid' or '.ask' if that is not what you mean"
        } else {
            "its first field; pass the field you mean"
        };
        self.sink
            .warning(range, format!("Type '{actual}' is not assignable to type '{accepted}': the server uses {used}."));
    }
}
