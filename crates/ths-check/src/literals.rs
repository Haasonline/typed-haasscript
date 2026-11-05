//! Turns literal tokens into the values literal types compare by.

use ths_syntax::{SyntaxKind, SyntaxToken};

use crate::types::Type;

/// The literal type of a literal token, or `None` if `token` is not one.
pub(crate) fn literal_type(token: &SyntaxToken) -> Option<Type> {
    let literal = match token.kind() {
        SyntaxKind::NilKeyword => Type::Nil,
        SyntaxKind::TrueKeyword => Type::BooleanLiteral(true),
        SyntaxKind::FalseKeyword => Type::BooleanLiteral(false),
        SyntaxKind::Number => Type::NumberLiteral(normalized_number(token.text())),
        SyntaxKind::String => Type::StringLiteral(short_string_value(token.text())),
        SyntaxKind::LongString => Type::StringLiteral(long_string_value(token.text())),
        _ => return None,
    };
    Some(literal)
}

/// One spelling per value, so `1`, `1.0` and `0x1` are the same literal.
pub(crate) fn normalized_number(text: &str) -> String {
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(digits) => i64::from_str_radix(digits, 16).ok().map(|value| value as f64),
        None => text.parse::<f64>().ok(),
    };
    match value {
        Some(value) => value.to_string(),
        None => text.to_string(),
    }
}

/// The value of a quoted string, with the common escapes decoded.
fn short_string_value(text: &str) -> String {
    let inner = &text[1..text.len().saturating_sub(1).max(1)];
    let mut value = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => value.push('\n'),
            Some('t') => value.push('\t'),
            Some('r') => value.push('\r'),
            Some(other) => value.push(other),
            None => {}
        }
    }
    value
}

/// The value of `[==[ ... ]==]`; a newline right after the opening bracket
/// is skipped, as in Lua.
fn long_string_value(text: &str) -> String {
    let level = text[1..].find('[').unwrap_or(0);
    let opening = level + 2;
    let closing = text.len().saturating_sub(level + 2).max(opening);
    let inner = &text[opening..closing];
    inner.strip_prefix("\r\n").or_else(|| inner.strip_prefix('\n')).unwrap_or(inner).to_string()
}
