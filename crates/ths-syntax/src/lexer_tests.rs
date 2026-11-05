use crate::SyntaxKind;
use crate::lexer::{LexErrorKind, lex};

fn tokens(source: &str) -> Vec<(SyntaxKind, &str)> {
    lex(source).tokens.iter().map(|token| (token.kind, &source[token.range])).collect()
}

fn error_kinds(source: &str) -> Vec<LexErrorKind> {
    lex(source).errors.iter().map(|error| error.kind).collect()
}

#[test]
fn token_texts_rebuild_the_source_exactly() {
    let source = "local x: number = 1 -- note\r\n--[==[ block ]==]\tprint('a\\'b', [[long]])\n";
    let rebuilt: String = tokens(source).iter().map(|(_, text)| *text).collect();
    assert_eq!(rebuilt, source);
}

#[test]
fn type_interface_and_as_lex_as_names() {
    let kinds: Vec<SyntaxKind> = tokens("type interface as").iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            SyntaxKind::Identifier,
            SyntaxKind::Whitespace,
            SyntaxKind::Identifier,
            SyntaxKind::Whitespace,
            SyntaxKind::Identifier,
        ]
    );
}

#[test]
fn keywords_lex_as_keywords() {
    assert_eq!(tokens("elseif"), [(SyntaxKind::ElseifKeyword, "elseif")]);
}

#[test]
fn long_string_closes_only_at_its_own_level() {
    assert_eq!(tokens("[==[ a ]] ]=] b ]==]"), [(SyntaxKind::LongString, "[==[ a ]] ]=] b ]==]")]);
}

#[test]
fn block_comment_spans_lines() {
    assert_eq!(
        tokens("--[[ one\ntwo ]]x"),
        [(SyntaxKind::BlockComment, "--[[ one\ntwo ]]"), (SyntaxKind::Identifier, "x"),]
    );
}

#[test]
fn line_comment_stops_before_the_newline() {
    assert_eq!(
        tokens("--[ not a block\nx"),
        [(SyntaxKind::LineComment, "--[ not a block"), (SyntaxKind::Newline, "\n"), (SyntaxKind::Identifier, "x"),]
    );
}

#[test]
fn carriage_return_line_feed_is_one_newline() {
    assert_eq!(tokens("\r\n"), [(SyntaxKind::Newline, "\r\n")]);
}

#[test]
fn valid_number_forms_lex_without_errors() {
    for source in ["3", "3.0", ".5", "3.", "1e10", "1E-3", "0xff", "0x1p4", "0x.8P-1"] {
        assert_eq!(tokens(source), [(SyntaxKind::Number, source)], "{source}");
        assert!(error_kinds(source).is_empty(), "{source}");
    }
}

#[test]
fn malformed_numbers_are_one_token_with_an_error() {
    for source in ["3x", "1e", "0x", "1..2"] {
        assert_eq!(tokens(source), [(SyntaxKind::Number, source)], "{source}");
        assert_eq!(error_kinds(source), [LexErrorKind::MalformedNumber], "{source}");
    }
}

#[test]
fn string_ending_at_a_newline_is_unterminated() {
    assert_eq!(error_kinds("'abc\nx"), [LexErrorKind::UnterminatedString]);
}

#[test]
fn escaped_quote_does_not_end_a_string() {
    assert_eq!(tokens(r#""a\"b""#), [(SyntaxKind::String, r#""a\"b""#)]);
}

#[test]
fn unclosed_long_bracket_is_reported() {
    assert_eq!(error_kinds("[[ open"), [LexErrorKind::UnterminatedLongBracket]);
}

#[test]
fn type_punctuation_lexes_as_single_tokens() {
    let kinds: Vec<SyntaxKind> = tokens("?|&").iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [SyntaxKind::Question, SyntaxKind::Pipe, SyntaxKind::Ampersand]);
}

#[test]
fn bang_equals_is_one_token() {
    assert_eq!(tokens("!="), [(SyntaxKind::BangEquals, "!=")]);
}

#[test]
fn shebang_first_line_is_trivia() {
    assert_eq!(
        tokens("#!anything\nx"),
        [(SyntaxKind::Shebang, "#!anything"), (SyntaxKind::Newline, "\n"), (SyntaxKind::Identifier, "x"),]
    );
}

#[test]
fn shebang_after_the_first_line_is_not_trivia() {
    assert_eq!(tokens("\n#!")[1].0, SyntaxKind::Hash);
}

#[test]
fn byte_order_mark_precedes_a_shebang() {
    assert_eq!(tokens("\u{FEFF}#!x"), [(SyntaxKind::ByteOrderMark, "\u{FEFF}"), (SyntaxKind::Shebang, "#!x")]);
}

#[test]
fn zero_width_space_is_whitespace() {
    assert_eq!(tokens("\u{200B}"), [(SyntaxKind::Whitespace, "\u{200B}")]);
}

#[test]
fn unknown_character_is_an_error_token() {
    assert_eq!(tokens("@"), [(SyntaxKind::Error, "@")]);
    assert_eq!(error_kinds("@"), [LexErrorKind::UnexpectedCharacter]);
}
