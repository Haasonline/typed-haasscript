//! Byte offsets into a document as the editor counts positions: lines
//! broken by `\n`, `\r\n` or `\r`, as the Language Server Protocol breaks
//! them, and characters counted in the encoding agreed at start-up.

use lsp_types::{Position, PositionEncodingKind, Range};
use rowan::{TextRange, TextSize};
use ths_check::Diagnostic;

/// How a position's character is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    /// In bytes, when the editor offers it.
    Utf8,
    /// In UTF-16 code units, which every editor understands.
    Utf16,
}

impl PositionEncoding {
    /// UTF-8 when the editor offers it, since offsets are bytes already;
    /// otherwise UTF-16, the protocol's default.
    pub fn chosen_from(offered: Option<&[PositionEncodingKind]>) -> PositionEncoding {
        let offers_utf8 = offered.is_some_and(|kinds| kinds.contains(&PositionEncodingKind::UTF8));
        if offers_utf8 { PositionEncoding::Utf8 } else { PositionEncoding::Utf16 }
    }

    pub fn kind(self) -> PositionEncodingKind {
        match self {
            PositionEncoding::Utf8 => PositionEncodingKind::UTF8,
            PositionEncoding::Utf16 => PositionEncodingKind::UTF16,
        }
    }
}

/// Where each line of a document starts.
pub struct LineIndex<'text> {
    text: &'text str,
    line_starts: Vec<usize>,
}

impl<'text> LineIndex<'text> {
    pub fn new(text: &'text str) -> LineIndex<'text> {
        let bytes = text.as_bytes();
        let mut line_starts = vec![0];
        for (index, byte) in bytes.iter().enumerate() {
            let ends_line = match byte {
                b'\n' => true,
                b'\r' => bytes.get(index + 1) != Some(&b'\n'),
                _ => false,
            };
            if ends_line {
                line_starts.push(index + 1);
            }
        }
        LineIndex { text, line_starts }
    }

    /// The position of the byte `offset`; past the end means the end.
    pub fn position(&self, offset: usize, encoding: PositionEncoding) -> Position {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let before = &self.text[self.line_starts[line]..offset];
        let character = match encoding {
            PositionEncoding::Utf8 => before.len(),
            PositionEncoding::Utf16 => before.encode_utf16().count(),
        };
        Position { line: saturated(line), character: saturated(character) }
    }

    pub fn range(&self, diagnostic: &Diagnostic, encoding: PositionEncoding) -> Range {
        self.text_range(diagnostic.range, encoding)
    }

    pub fn text_range(&self, range: TextRange, encoding: PositionEncoding) -> Range {
        Range {
            start: self.position(usize::from(range.start()), encoding),
            end: self.position(usize::from(range.end()), encoding),
        }
    }

    /// The byte offset of `position`; past the end of its line means the
    /// line's end, and past the last line the end of the text.
    pub fn offset(&self, position: Position, encoding: PositionEncoding) -> TextSize {
        let Some(&line_start) = self.line_starts.get(position.line as usize) else {
            return text_size(self.text.len());
        };
        let line_end = self.line_starts.get(position.line as usize + 1).copied().unwrap_or(self.text.len());
        let line = self.text[line_start..line_end].trim_end_matches(['\r', '\n']);
        let mut counted = 0;
        for (index, character) in line.char_indices() {
            if counted >= position.character as usize {
                return text_size(line_start + index);
            }
            counted += match encoding {
                PositionEncoding::Utf8 => character.len_utf8(),
                PositionEncoding::Utf16 => character.len_utf16(),
            };
        }
        text_size(line_start + line.len())
    }
}

fn text_size(offset: usize) -> TextSize {
    TextSize::from(u32::try_from(offset).unwrap_or(u32::MAX))
}

fn saturated(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, offset: usize, encoding: PositionEncoding) -> (u32, u32) {
        let position = LineIndex::new(text).position(offset, encoding);
        (position.line, position.character)
    }

    #[test]
    fn lines_and_characters_count_from_zero() {
        assert_eq!(at("abc", 0, PositionEncoding::Utf16), (0, 0));
        assert_eq!(at("a\nbc", 3, PositionEncoding::Utf16), (1, 1));
    }

    #[test]
    fn every_line_break_of_the_protocol_ends_a_line() {
        let text = "a\r\nb\rc\nd";
        assert_eq!(at(text, text.find('b').unwrap(), PositionEncoding::Utf16), (1, 0));
        assert_eq!(at(text, text.find('c').unwrap(), PositionEncoding::Utf16), (2, 0));
        assert_eq!(at(text, text.find('d').unwrap(), PositionEncoding::Utf16), (3, 0));
    }

    #[test]
    fn characters_are_counted_in_the_agreed_encoding() {
        // U+00E9 is 2 bytes and 1 UTF-16 unit; U+1F600 is 4 bytes and 2 units.
        let text = "\u{e9}\u{1f600}x";
        let offset = text.find('x').unwrap();
        assert_eq!(at(text, offset, PositionEncoding::Utf8), (0, 6));
        assert_eq!(at(text, offset, PositionEncoding::Utf16), (0, 3));
    }

    #[test]
    fn an_offset_past_the_end_is_the_end() {
        assert_eq!(at("ab\n", 10, PositionEncoding::Utf16), (1, 0));
    }

    #[test]
    fn a_position_turns_back_into_its_offset() {
        let text = "a\r\n\u{1f600}bc\nd";
        let index = LineIndex::new(text);
        for encoding in [PositionEncoding::Utf8, PositionEncoding::Utf16] {
            let offset = text.find('c').unwrap();
            let position = index.position(offset, encoding);
            assert_eq!(usize::from(index.offset(position, encoding)), offset);
        }
        let past_the_line = Position { line: 0, character: 40 };
        assert_eq!(usize::from(index.offset(past_the_line, PositionEncoding::Utf16)), 1);
    }

    #[test]
    fn utf8_is_chosen_only_when_offered() {
        assert_eq!(PositionEncoding::chosen_from(None), PositionEncoding::Utf16);
        let offered = [PositionEncodingKind::UTF16, PositionEncodingKind::UTF8];
        assert_eq!(PositionEncoding::chosen_from(Some(&offered)), PositionEncoding::Utf8);
    }
}
