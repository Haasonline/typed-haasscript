//! Indexing a function, a number or a boolean stops the script with
//! "attempt to index a … value", reading or writing (verified on the
//! TradeServer): `CurrentPrice[1]` for
//! `CurrentPrice()[1]` is the usual slip, since a command's name is a
//! plain function. A string indexed with brackets gives nil, so it is not
//! reported here. Field reads (`number.close`) are reported by the member
//! check.

use rowan::TextRange;

use super::Checker;
use crate::types::Type;

impl Checker<'_> {
    /// Reports indexing `owner` with brackets when every value it may hold
    /// is a function, a number or a boolean.
    pub(super) fn check_indexable(&mut self, owner: &Type, range: TextRange) {
        let members: Vec<Type> =
            self.expand(&owner.without_nil()).members().iter().map(|member| self.expand(member)).collect();
        let unindexable = !members.is_empty()
            && members.iter().all(|member| {
                matches!(
                    member,
                    Type::Function(_) | Type::Number | Type::NumberLiteral(_) | Type::Boolean | Type::BooleanLiteral(_)
                )
            });
        if !unindexable {
            return;
        }
        let hint = if members.iter().any(|member| matches!(member, Type::Function(_))) {
            " A command's name is a function; call it first."
        } else {
            ""
        };
        self.sink
            .error(range, format!("A value of type '{owner}' cannot be indexed: the server stops the script.{hint}"));
    }
}
