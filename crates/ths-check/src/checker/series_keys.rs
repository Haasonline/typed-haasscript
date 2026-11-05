//! A series used as a key of a Lua table is a key of its own, not the
//! number it holds, so the entry is silently never found: `t[Max(1, 2)]`
//! is nil although `t[2]` is set, and `t[Max(1, 1)] = 'a'` leaves `#t` at
//! 0 (verified on the TradeServer). The
//! math helpers return a series even for plain numbers, so this is easy to
//! hit with no error at runtime.

use rowan::TextRange;

use super::Checker;
use super::series::is_series;
use crate::types::Type;

const SERIES_KEY_MESSAGE: &str = "A series used as a table key is a key of its own, not its number, so the entry is never found; use ArrayGet(value, 1) for the number.";

impl Checker<'_> {
    /// Reports `key`, the key of a Lua table, when it may be a series.
    pub(super) fn check_table_key(&mut self, key: &Type, range: TextRange) {
        let may_be_series = self.expand(key).members().iter().any(|member| is_series(&self.expand(member)));
        if may_be_series {
            self.sink.error(range, SERIES_KEY_MESSAGE.to_string());
        }
    }
}
