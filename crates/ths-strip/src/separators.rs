//! Decides where blanking would let Lua join two statements.
//!
//! Lua has no statement terminator: a statement starting with `(` continues
//! the expression before it, so `local x = y` and `(z)()` on the next line
//! read as `y(z)()`. In typed source a removed declaration or cast tail can
//! be the only thing between such a pair.

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::erasable_ranges::Erasure;

/// Ranges among `removed` that must keep a statement separator. When several
/// removed ranges sit in a row before the `(`, only the last one needs it.
pub(crate) fn ranges_needing_a_separator(root: &SyntaxNode, removed: &[Erasure]) -> Vec<TextRange> {
    let significant: Vec<(SyntaxKind, TextRange)> = root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| (token.kind(), token.text_range()))
        .filter(|(_, range)| !removed.iter().any(|erasure| erasure.range.contains_range(*range)))
        .collect();
    removed
        .iter()
        .enumerate()
        .filter(|(_, erasure)| erasure.ends_a_statement)
        .filter(|(index, erasure)| {
            let next_token = significant.iter().find(|(_, range)| range.start() >= erasure.range.end());
            let Some((SyntaxKind::LeftParenthesis, parenthesis)) = next_token else {
                return false;
            };
            let another_removal_follows =
                removed.get(index + 1).is_some_and(|next| next.range.start() < parenthesis.start());
            !another_removal_follows
        })
        .map(|(_, erasure)| erasure.range)
        .collect()
}
