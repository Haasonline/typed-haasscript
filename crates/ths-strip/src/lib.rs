//! Turns Typed HaasScript into plain HaasScript by blanking out everything
//! that only exists for the type checker. Every character in a removed range
//! becomes a space and newlines stay, so each line and column of the output
//! matches the source. A bundle's lines cannot match anyway, so
//! `strip_compact` cuts the ranges out instead, for `bundle` to join a
//! trade script and its modules.

mod bundle;
mod erasable_ranges;
mod separators;

use rowan::TextRange;
use ths_syntax::{Dialect, ParseError, parse};

pub use crate::bundle::{Binding, BundleModule, BundlePart, bundle};
use crate::erasable_ranges::{Erasure, erasures};
use crate::separators::ranges_needing_a_separator;

/// What replaces the first character of a removed range that kept two
/// statements apart.
const STATEMENT_SEPARATOR: char = ';';
const BLANK: char = ' ';

/// Strips a typed source, or returns its syntax errors.
pub fn strip(source: &str) -> Result<String, Vec<ParseError>> {
    let (removed, separated) = removed_ranges(source)?;
    Ok(blank_out(source, &removed, &separated))
}

/// Strips a typed source as `strip` does, cutting each removed range out
/// instead of blanking it, and with it the line of a statement that was
/// alone on its line.
pub fn strip_compact(source: &str) -> Result<String, Vec<ParseError>> {
    let (removed, separated) = removed_ranges(source)?;
    Ok(cut_out(source, &removed, &separated))
}

fn removed_ranges(source: &str) -> Result<(Vec<Erasure>, Vec<TextRange>), Vec<ParseError>> {
    let parsed = parse(source, Dialect::Typed);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    let root = parsed.syntax_node();
    let removed = erasures(&root);
    let separated = ranges_needing_a_separator(&root, &removed);
    Ok((removed, separated))
}

fn cut_out(source: &str, removed: &[Erasure], separated: &[TextRange]) -> String {
    let mut output = String::with_capacity(source.len());
    let mut copied_up_to = 0;
    for erasure in removed {
        let (mut start, mut end) = (usize::from(erasure.range.start()), usize::from(erasure.range.end()));
        let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1).max(copied_up_to);
        output.push_str(&source[copied_up_to..line_start]);
        copied_up_to = line_start;
        if let Some(line_end) = rest_of_line_if_alone(source, &output, line_start, start, end) {
            // Drops the indentation already written on the line.
            output.truncate(output.rfind('\n').map_or(0, |index| index + 1));
            (start, end) = (line_start, line_end);
        }
        output.push_str(&source[copied_up_to..start]);
        if let Some(replacement) = cut_replacement(&output, &source[end..], separated.contains(&erasure.range)) {
            output.push(replacement);
        }
        copied_up_to = end;
    }
    output.push_str(&source[copied_up_to..]);
    output
}

/// What takes the place of a range cut out between `before` and `after`:
/// the statement separator when the range kept two statements apart, a
/// blank when cutting it would join two names, and otherwise nothing.
fn cut_replacement(before: &str, after: &str, needs_separator: bool) -> Option<char> {
    let joins_names = before.chars().next_back().is_some_and(is_name_character)
        && after.chars().next().is_some_and(is_name_character);
    if needs_separator {
        Some(STATEMENT_SEPARATOR)
    } else if joins_names {
        Some(BLANK)
    } else {
        None
    }
}

/// Where the line holding `start..end` ends, its line break included,
/// when nothing but spaces would be left on it: in the output written on
/// the line, in the source from `line_start` to `start`, and after `end`.
/// None of it can be inside a string: the range starts a node and the
/// rest is blank.
fn rest_of_line_if_alone(source: &str, output: &str, line_start: usize, start: usize, end: usize) -> Option<usize> {
    let is_blank = |text: &str| text.chars().all(|character| matches!(character, ' ' | '\t' | '\r' | '\n'));
    let written_on_line = &output[output.rfind('\n').map_or(0, |index| index + 1)..];
    let line_end = source[end..].find('\n').map_or(source.len(), |index| end + index + 1);
    let alone = is_blank(written_on_line) && is_blank(&source[line_start..start]) && is_blank(&source[end..line_end]);
    alone.then_some(line_end)
}

fn is_name_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn blank_out(source: &str, removed: &[Erasure], separated: &[TextRange]) -> String {
    let mut output = String::with_capacity(source.len());
    let mut copied_up_to = 0;
    for erasure in removed {
        let start = usize::from(erasure.range.start());
        let end = usize::from(erasure.range.end());
        output.push_str(&source[copied_up_to..start]);
        let needs_separator = separated.contains(&erasure.range);
        for (index, character) in source[start..end].chars().enumerate() {
            output.push(match character {
                '\r' | '\n' => character,
                _ if index == 0 && needs_separator => STATEMENT_SEPARATOR,
                _ => BLANK,
            });
        }
        copied_up_to = end;
    }
    output.push_str(&source[copied_up_to..]);
    output
}

#[cfg(test)]
mod tests;
