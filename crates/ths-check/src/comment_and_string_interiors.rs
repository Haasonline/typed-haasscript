//! Where a cursor is inside a comment or a string, so an editor offers no
//! names there. A cursor right before the token is outside it, and right
//! after it is outside too, unless the token is still open: a line comment,
//! or a string or long bracket not closed yet, which typing extends.

use rowan::{TextRange, TextSize};
use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

const LINE_COMMENT_MARKER: &str = "--";
const ESCAPE: char = '\\';

/// For each comment and string, the cursor offsets inside it, as a
/// half-open range.
pub(crate) fn interiors(root: &SyntaxNode) -> Vec<TextRange> {
    root.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter_map(|token| interior(&token))
        .collect()
}

fn interior(token: &SyntaxToken) -> Option<TextRange> {
    let text = token.text();
    let is_open = match token.kind() {
        SyntaxKind::LineComment => true,
        SyntaxKind::String => !is_closed_short_string(text),
        SyntaxKind::LongString => !is_closed_long_bracket(text),
        SyntaxKind::BlockComment => !is_closed_long_bracket(text.strip_prefix(LINE_COMMENT_MARKER).unwrap_or(text)),
        _ => return None,
    };
    let range = token.text_range();
    let end = if is_open { range.end() + TextSize::from(1) } else { range.end() };
    Some(TextRange::new(range.start() + TextSize::from(1), end))
}

/// Ends with its opening quote, not escaped.
fn is_closed_short_string(text: &str) -> bool {
    let mut characters = text.chars();
    let (Some(quote), Some(last)) = (characters.next(), characters.next_back()) else { return false };
    if last != quote {
        return false;
    }
    let escapes = characters.rev().take_while(|character| *character == ESCAPE).count();
    escapes % 2 == 0
}

/// `[==[ … ]==]`: ends with the close of the level it opens with.
fn is_closed_long_bracket(text: &str) -> bool {
    let level = text.chars().skip(1).take_while(|character| *character == '=').count();
    let opening = level + 2;
    let closing = format!("]{}]", "=".repeat(level));
    text.len() >= opening + closing.len() && text.ends_with(&closing)
}
