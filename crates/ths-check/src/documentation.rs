//! A declaration's documentation: the `--` comment lines right above it,
//! each on a line of its own, with no blank line between, as the built-in
//! declarations and most Lua code write it.

use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

const LINE_COMMENT_MARKER: &str = "--";

pub(crate) fn leading_comment(statement: &SyntaxNode) -> Option<String> {
    let mut lines = Vec::new();
    let mut newlines_since_comment = 0;
    let mut token = statement.first_token()?.prev_token();
    while let Some(current) = token {
        match current.kind() {
            SyntaxKind::Whitespace => {}
            SyntaxKind::Newline => {
                newlines_since_comment += 1;
                if newlines_since_comment > 1 {
                    break;
                }
            }
            SyntaxKind::LineComment if starts_its_line(&current) => {
                let text = current.text().strip_prefix(LINE_COMMENT_MARKER).unwrap_or(current.text());
                lines.push(text.strip_prefix(' ').unwrap_or(text).trim_end().to_string());
                newlines_since_comment = 0;
            }
            _ => break,
        }
        token = current.prev_token();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

/// Whether only spaces come before `comment` on its line, so it is not a
/// note trailing the code above.
fn starts_its_line(comment: &SyntaxToken) -> bool {
    let mut token = comment.prev_token();
    while let Some(current) = token {
        match current.kind() {
            SyntaxKind::Whitespace => token = current.prev_token(),
            SyntaxKind::Newline => return true,
            _ => return false,
        }
    }
    true
}
