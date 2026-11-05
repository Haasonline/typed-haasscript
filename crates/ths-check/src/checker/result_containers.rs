//! A TradeServer command's result container (what `CurrentPrice()` or
//! `MACD()` returns) is an engine value, not a Lua table: it has numbered
//! and named fields, but the rules for Lua tables do not apply to it.

use super::Checker;
use crate::types::Type;

impl Checker<'_> {
    /// Whether `value` is a command's result container, which is no Lua
    /// table.
    pub(super) fn is_container_type(&self, value: &Type) -> bool {
        matches!(value, Type::Reference(name, _) if self.environment.is_container(name))
    }
}
