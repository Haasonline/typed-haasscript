//! The types of chosen expressions, kept for a caller that needs more than
//! diagnostics, such as the value a custom command gives `DefineOutput`.

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use crate::types::Type;

#[derive(Debug, Default)]
pub(crate) struct RecordedTypes {
    wanted: HashSet<TextRange>,
    found: HashMap<TextRange, Type>,
}

impl RecordedTypes {
    pub(crate) fn of(ranges: impl IntoIterator<Item = TextRange>) -> Self {
        RecordedTypes { wanted: ranges.into_iter().collect(), found: HashMap::new() }
    }

    /// Keeps `value` when the expression at `range` is wanted. An
    /// expression checked more than once, as in a loop, keeps every type
    /// it had.
    pub(super) fn note(&mut self, range: TextRange, value: &Type) {
        if self.wanted.is_empty() || !self.wanted.contains(&range) {
            return;
        }
        let kept = match self.found.remove(&range) {
            Some(earlier) => Type::union([earlier, value.clone()]),
            None => value.clone(),
        };
        self.found.insert(range, kept);
    }

    /// Wants the expression at `range` too.
    pub(super) fn want(&mut self, range: TextRange) {
        self.wanted.insert(range);
    }

    /// The type the expression at `range` had, no longer wanted.
    pub(super) fn take(&mut self, range: TextRange) -> Option<Type> {
        self.wanted.remove(&range);
        self.found.remove(&range)
    }

    /// The type of the expression at `range`, when it was checked.
    pub(crate) fn get(&self, range: TextRange) -> Option<&Type> {
        self.found.get(&range)
    }
}
