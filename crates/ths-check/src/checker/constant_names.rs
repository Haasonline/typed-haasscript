//! An enumeration constant compared with a string compares the constant's
//! name, case-sensitively, in either order: `GetType(1) ==
//! 'NumberDataType'` and `SignalLong == 'SignalLong'` are true, any other
//! text is unequal, and colors never equal a string (verified on the
//! TradeServer). So a string literal
//! naming a constant of the category overlaps it, another literal does
//! not (`GetType(name) == 'string'` is always false), and a string that
//! is not a literal may hold a name.

use super::Checker;
use crate::engine_types::{COLOR_TYPE, ENUM_TYPE};
use crate::types::Type;

impl Checker<'_> {
    /// Whether `left` and `right`, expanded, are an enumeration constant
    /// and a string that may be its name.
    pub(super) fn constant_may_equal_name(&self, left: &Type, right: &Type) -> bool {
        match (left, right) {
            (Type::Opaque(category), text) | (text, Type::Opaque(category)) if self.compares_by_name(category) => {
                match text {
                    Type::String => true,
                    Type::StringLiteral(name) => self.names_constant_of(name, category),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn compares_by_name(&self, category: &str) -> bool {
        category != COLOR_TYPE && self.environment.opaque_extends(category, ENUM_TYPE)
    }

    /// Whether `name` is a declared global constant of `category`.
    fn names_constant_of(&self, name: &str, category: &str) -> bool {
        self.scopes.lookup_declared_global(name).is_some_and(
            |id| matches!(self.expand(self.scopes.declared_type(id)), Type::Opaque(declared) if declared == category),
        )
    }
}
