//! Splits source text into tokens without losing a single byte: every
//! space, newline and comment becomes a token, so the token texts joined
//! together are exactly the source.

use std::fmt;

use rowan::{TextRange, TextSize};

use crate::syntax_kind::SyntaxKind;

const ZERO_WIDTH_SPACE: &[u8] = "\u{200B}".as_bytes();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    UnexpectedCharacter,
    UnterminatedString,
    UnterminatedLongBracket,
    MalformedNumber,
}

impl fmt::Display for LexErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            LexErrorKind::UnexpectedCharacter => "unexpected character",
            LexErrorKind::UnterminatedString => "unfinished string",
            LexErrorKind::UnterminatedLongBracket => "unfinished long string or comment",
            LexErrorKind::MalformedNumber => "malformed number",
        };
        formatter.write_str(message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub range: TextRange,
}

#[derive(Debug, Default)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

pub fn lex(source: &str) -> LexOutput {
    let mut lexer = Lexer { source: source.as_bytes(), position: 0, output: LexOutput::default() };
    lexer.leading_trivia();
    while lexer.position < lexer.source.len() {
        lexer.next_token();
    }
    lexer.output
}

struct Lexer<'source> {
    source: &'source [u8],
    position: usize,
    output: LexOutput,
}

impl Lexer<'_> {
    fn peek(&self, offset: usize) -> Option<u8> {
        self.source.get(self.position + offset).copied()
    }

    fn range_from(&self, start: usize) -> TextRange {
        TextRange::new(text_size(start), text_size(self.position))
    }

    fn push_token(&mut self, kind: SyntaxKind, start: usize) {
        let range = self.range_from(start);
        self.output.tokens.push(Token { kind, range });
    }

    fn push_error(&mut self, kind: LexErrorKind, start: usize) {
        let range = self.range_from(start);
        self.output.errors.push(LexError { kind, range });
    }

    /// A byte-order mark, then a `#!` first line, may only open the source.
    fn leading_trivia(&mut self) {
        const BYTE_ORDER_MARK: &[u8] = "\u{FEFF}".as_bytes();
        if self.source.starts_with(BYTE_ORDER_MARK) {
            self.position = BYTE_ORDER_MARK.len();
            self.push_token(SyntaxKind::ByteOrderMark, 0);
        }
        if self.source[self.position..].starts_with(b"#!") {
            let start = self.position;
            self.eat_while(|byte| !matches!(byte, b'\r' | b'\n'));
            self.push_token(SyntaxKind::Shebang, start);
        }
    }

    fn next_token(&mut self) {
        let start = self.position;
        let first = self.source[start];
        // The TradeServer deletes zero-width spaces before compiling, so
        // they never cause an error there. Treating one as whitespace matches
        // that everywhere except inside a name, where the server would join
        // the two halves.
        if self.source[start..].starts_with(ZERO_WIDTH_SPACE) {
            self.position += ZERO_WIDTH_SPACE.len();
            self.push_token(SyntaxKind::Whitespace, start);
            return;
        }
        let kind = match first {
            b' ' | b'\t' | 0x0B | 0x0C => {
                self.eat_while(|byte| matches!(byte, b' ' | b'\t' | 0x0B | 0x0C));
                SyntaxKind::Whitespace
            }
            b'\r' | b'\n' => {
                self.eat_newline();
                SyntaxKind::Newline
            }
            b'-' if self.peek(1) == Some(b'-') => self.comment(start),
            b'"' | b'\'' => self.short_string(start, first),
            b'[' if self.long_bracket_level().is_some() => {
                self.long_bracket(start);
                SyntaxKind::LongString
            }
            b'0'..=b'9' => self.number(start),
            b'.' if self.peek(1).is_some_and(|byte| byte.is_ascii_digit()) => self.number(start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                self.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
                let name = std::str::from_utf8(&self.source[start..self.position]).expect("names are ASCII");
                SyntaxKind::keyword_from_name(name).unwrap_or(SyntaxKind::Identifier)
            }
            _ => self.punctuation(start),
        };
        self.push_token(kind, start);
    }

    fn eat_while(&mut self, predicate: impl Fn(u8) -> bool) {
        while self.peek(0).is_some_and(&predicate) {
            self.position += 1;
        }
    }

    /// Consumes one line ending: `\n`, `\r`, `\r\n` or `\n\r`, as Lua does.
    fn eat_newline(&mut self) {
        let first = self.source[self.position];
        self.position += 1;
        if let Some(second) = self.peek(0)
            && matches!(second, b'\r' | b'\n')
            && second != first
        {
            self.position += 1;
        }
    }

    fn comment(&mut self, start: usize) -> SyntaxKind {
        self.position += 2;
        if self.long_bracket_level().is_some() {
            self.long_bracket(start);
            return SyntaxKind::BlockComment;
        }
        self.eat_while(|byte| !matches!(byte, b'\r' | b'\n'));
        SyntaxKind::LineComment
    }

    /// If a long bracket (`[[`, `[=[`, `[==[`, ...) opens at the current
    /// position, returns its level: the number of `=` signs.
    fn long_bracket_level(&self) -> Option<usize> {
        if self.peek(0) != Some(b'[') {
            return None;
        }
        let mut level = 0;
        while self.peek(1 + level) == Some(b'=') {
            level += 1;
        }
        (self.peek(1 + level) == Some(b'[')).then_some(level)
    }

    /// Consumes the long bracket opening at the current position through its
    /// matching close of the same level.
    fn long_bracket(&mut self, start: usize) {
        let level = self.long_bracket_level().expect("caller checked for a long bracket");
        self.position += 2 + level;
        while self.position < self.source.len() {
            if self.source[self.position] == b']' {
                let mut equals = 0;
                while self.peek(1 + equals) == Some(b'=') {
                    equals += 1;
                }
                if equals == level && self.peek(1 + equals) == Some(b']') {
                    self.position += 2 + level;
                    return;
                }
            }
            self.position += 1;
        }
        self.push_error(LexErrorKind::UnterminatedLongBracket, start);
    }

    fn short_string(&mut self, start: usize, quote: u8) -> SyntaxKind {
        self.position += 1;
        loop {
            match self.peek(0) {
                None | Some(b'\r' | b'\n') => {
                    self.push_error(LexErrorKind::UnterminatedString, start);
                    break;
                }
                Some(b'\\') => {
                    self.position += 1;
                    match self.peek(0) {
                        Some(b'\r' | b'\n') => self.eat_newline(),
                        Some(_) => self.position += 1,
                        None => {}
                    }
                }
                Some(byte) => {
                    self.position += 1;
                    if byte == quote {
                        break;
                    }
                }
            }
        }
        SyntaxKind::String
    }

    /// Lua 5.2 numerals: decimal with optional fraction and `e` exponent, or
    /// hexadecimal with optional fraction and `p` exponent. Like Lua, any
    /// trailing letters or digits are swallowed into one malformed token.
    fn number(&mut self, start: usize) -> SyntaxKind {
        let is_hexadecimal = self.peek(0) == Some(b'0') && matches!(self.peek(1), Some(b'x' | b'X'));
        let exponent_markers: [u8; 2] = if is_hexadecimal {
            self.position += 2;
            *b"pP"
        } else {
            *b"eE"
        };
        loop {
            match self.peek(0) {
                Some(byte) if exponent_markers.contains(&byte) => {
                    self.position += 1;
                    if matches!(self.peek(0), Some(b'+' | b'-')) {
                        self.position += 1;
                    }
                }
                Some(byte) if byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' => {
                    self.position += 1;
                }
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.source[start..self.position]).expect("ASCII");
        if !is_valid_number(text) {
            self.push_error(LexErrorKind::MalformedNumber, start);
        }
        SyntaxKind::Number
    }

    fn punctuation(&mut self, start: usize) -> SyntaxKind {
        let first = self.source[start];
        let second = self.peek(1);
        let (kind, length) = match (first, second) {
            (b'.', Some(b'.')) if self.peek(2) == Some(b'.') => (SyntaxKind::TripleDot, 3),
            (b'.', Some(b'.')) => (SyntaxKind::DoubleDot, 2),
            (b'.', _) => (SyntaxKind::Dot, 1),
            (b'=', Some(b'=')) => (SyntaxKind::EqualsEquals, 2),
            (b'=', Some(b'>')) => (SyntaxKind::FatArrow, 2),
            (b'=', _) => (SyntaxKind::Equals, 1),
            (b'~', Some(b'=')) => (SyntaxKind::TildeEquals, 2),
            (b'!', Some(b'=')) => (SyntaxKind::BangEquals, 2),
            (b'<', Some(b'=')) => (SyntaxKind::LessEquals, 2),
            (b'<', _) => (SyntaxKind::Less, 1),
            (b'>', Some(b'=')) => (SyntaxKind::GreaterEquals, 2),
            (b'>', _) => (SyntaxKind::Greater, 1),
            (b':', Some(b':')) => (SyntaxKind::DoubleColon, 2),
            (b':', _) => (SyntaxKind::Colon, 1),
            (b'+', _) => (SyntaxKind::Plus, 1),
            (b'-', _) => (SyntaxKind::Minus, 1),
            (b'*', _) => (SyntaxKind::Star, 1),
            (b'/', _) => (SyntaxKind::Slash, 1),
            (b'%', _) => (SyntaxKind::Percent, 1),
            (b'^', _) => (SyntaxKind::Caret, 1),
            (b'#', _) => (SyntaxKind::Hash, 1),
            (b'&', _) => (SyntaxKind::Ampersand, 1),
            (b'|', _) => (SyntaxKind::Pipe, 1),
            (b'?', _) => (SyntaxKind::Question, 1),
            (b'(', _) => (SyntaxKind::LeftParenthesis, 1),
            (b')', _) => (SyntaxKind::RightParenthesis, 1),
            (b'{', _) => (SyntaxKind::LeftBrace, 1),
            (b'}', _) => (SyntaxKind::RightBrace, 1),
            (b'[', _) => (SyntaxKind::LeftBracket, 1),
            (b']', _) => (SyntaxKind::RightBracket, 1),
            (b';', _) => (SyntaxKind::Semicolon, 1),
            (b',', _) => (SyntaxKind::Comma, 1),
            _ => {
                // Consume a whole UTF-8 character so ranges stay on
                // character boundaries.
                self.position += utf8_length(first);
                self.push_error(LexErrorKind::UnexpectedCharacter, start);
                return SyntaxKind::Error;
            }
        };
        self.position += length;
        kind
    }
}

fn is_valid_number(text: &str) -> bool {
    let (digits, is_digit, exponent_markers): (&str, fn(char) -> bool, [char; 2]) =
        match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(rest) => (rest, |character| character.is_ascii_hexdigit(), ['p', 'P']),
            None => (text, |character| character.is_ascii_digit(), ['e', 'E']),
        };
    let (mantissa, exponent) = match digits.find(exponent_markers) {
        Some(index) => (&digits[..index], Some(&digits[index + 1..])),
        None => (digits, None),
    };
    let mut parts = mantissa.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    let mantissa_is_valid =
        !(whole.is_empty() && fraction.is_empty()) && whole.chars().all(is_digit) && fraction.chars().all(is_digit);
    let exponent_is_valid = exponent.is_none_or(|exponent| {
        let unsigned = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !unsigned.is_empty() && unsigned.chars().all(|character| character.is_ascii_digit())
    });
    mantissa_is_valid && exponent_is_valid
}

fn utf8_length(first_byte: u8) -> usize {
    match first_byte {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

fn text_size(offset: usize) -> TextSize {
    TextSize::try_from(offset).expect("source larger than 4 GiB")
}
