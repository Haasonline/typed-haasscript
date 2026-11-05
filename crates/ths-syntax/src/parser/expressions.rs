//! Expression grammar: Lua 5.2 operators and precedence, plus MoonSharp's
//! `!=` and `|x| expr` lambdas.

use super::Parser;
use super::contextual_keyword::ContextualKeyword;
use super::statements;
use super::types;
use crate::syntax_kind::SyntaxKind;

/// Left binding power of `as`: the same as comparisons, as in TypeScript,
/// so `a + b as T` is `(a + b) as T`.
const CAST_BINDING_POWER: u8 = 5;

/// The outermost form of a parsed suffixed expression, which decides
/// whether it can be assigned to or stand alone as a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionKind {
    Name,
    Parenthesized,
    Field,
    Index,
    Call,
    MethodCall,
    Instantiation,
}

/// Binding power of unary operators: tighter than every binary operator
/// except `^`, so `-x ^ 2` is `-(x ^ 2)`.
const UNARY_BINDING_POWER: u8 = 14;

/// Left and right binding powers. Higher binds tighter; right lower than
/// left makes an operator right-associative (`..` and `^`).
fn binary_binding_power(kind: SyntaxKind) -> Option<(u8, u8)> {
    let powers = match kind {
        SyntaxKind::OrKeyword => (1, 2),
        SyntaxKind::AndKeyword => (3, 4),
        SyntaxKind::Less
        | SyntaxKind::Greater
        | SyntaxKind::LessEquals
        | SyntaxKind::GreaterEquals
        | SyntaxKind::TildeEquals
        | SyntaxKind::BangEquals
        | SyntaxKind::EqualsEquals => (5, 6),
        SyntaxKind::DoubleDot => (9, 8),
        SyntaxKind::Plus | SyntaxKind::Minus => (10, 11),
        SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent => (12, 13),
        SyntaxKind::Caret => (17, 16),
        _ => return None,
    };
    Some(powers)
}

fn is_unary_operator(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::NotKeyword | SyntaxKind::Minus | SyntaxKind::Hash)
}

fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Number
            | SyntaxKind::String
            | SyntaxKind::LongString
            | SyntaxKind::NilKeyword
            | SyntaxKind::TrueKeyword
            | SyntaxKind::FalseKeyword
    )
}

pub(super) fn can_start_suffixed_expression(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Identifier | SyntaxKind::LeftParenthesis)
}

pub(super) fn can_start_expression(kind: SyntaxKind) -> bool {
    can_start_suffixed_expression(kind)
        || is_unary_operator(kind)
        || is_literal(kind)
        || matches!(
            kind,
            SyntaxKind::TripleDot | SyntaxKind::FunctionKeyword | SyntaxKind::LeftBrace | SyntaxKind::Pipe
        )
}

pub(super) fn expression(parser: &mut Parser) {
    binary_expression(parser, 0);
}

pub(super) fn expression_list(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ExpressionList);
    expression(parser);
    while parser.eat(SyntaxKind::Comma) {
        expression(parser);
    }
    parser.finish_node();
}

fn binary_expression(parser: &mut Parser, minimum_binding_power: u8) {
    let checkpoint = parser.checkpoint();
    if is_unary_operator(parser.current()) {
        parser.start_node(SyntaxKind::UnaryExpression);
        parser.bump();
        binary_expression(parser, UNARY_BINDING_POWER);
        parser.finish_node();
    } else if !simple_expression(parser) {
        return;
    }
    loop {
        if parser.is_typed() && parser.nth_is_keyword(0, ContextualKeyword::As) {
            if CAST_BINDING_POWER < minimum_binding_power {
                break;
            }
            parser.start_node_at(checkpoint, SyntaxKind::CastExpression);
            parser.bump();
            types::type_expression(parser);
            parser.finish_node();
            continue;
        }
        let Some((left, right)) = binary_binding_power(parser.current()) else {
            break;
        };
        if left < minimum_binding_power {
            break;
        }
        parser.start_node_at(checkpoint, SyntaxKind::BinaryExpression);
        parser.bump();
        binary_expression(parser, right);
        parser.finish_node();
    }
}

/// Returns false, after reporting an error, when no expression starts here.
fn simple_expression(parser: &mut Parser) -> bool {
    let kind = parser.current();
    if is_literal(kind) {
        parser.start_node(SyntaxKind::LiteralExpression);
        parser.bump();
        parser.finish_node();
        return true;
    }
    match kind {
        SyntaxKind::TripleDot => {
            parser.start_node(SyntaxKind::VarargExpression);
            parser.bump();
            parser.finish_node();
        }
        SyntaxKind::FunctionKeyword => {
            parser.start_node(SyntaxKind::FunctionExpression);
            parser.bump();
            function_body(parser);
            parser.finish_node();
        }
        SyntaxKind::LeftBrace => table_constructor(parser),
        SyntaxKind::Pipe => lambda_expression(parser),
        _ if can_start_suffixed_expression(kind) => {
            suffixed_expression(parser);
        }
        _ => {
            parser.error("expected an expression".to_string());
            return false;
        }
    }
    true
}

/// A name or parenthesized expression followed by any number of `.name`,
/// `[index]`, `:method(arguments)`, `(arguments)` and, in the typed
/// dialect, `<<types>>` suffixes.
pub(super) fn suffixed_expression(parser: &mut Parser) -> Option<ExpressionKind> {
    let checkpoint = parser.checkpoint();
    let mut outermost = match parser.current() {
        SyntaxKind::Identifier => {
            parser.start_node(SyntaxKind::NameExpression);
            parser.bump();
            parser.finish_node();
            ExpressionKind::Name
        }
        SyntaxKind::LeftParenthesis => {
            parser.start_node(SyntaxKind::ParenthesizedExpression);
            parser.bump();
            expression(parser);
            parser.expect(SyntaxKind::RightParenthesis, "')'");
            parser.finish_node();
            ExpressionKind::Parenthesized
        }
        _ => {
            parser.error("expected a name or '('".to_string());
            return None;
        }
    };
    loop {
        outermost = match parser.current() {
            SyntaxKind::Dot => {
                parser.start_node_at(checkpoint, SyntaxKind::FieldExpression);
                parser.bump();
                parser.expect(SyntaxKind::Identifier, "a field name after '.'");
                ExpressionKind::Field
            }
            SyntaxKind::LeftBracket => {
                parser.start_node_at(checkpoint, SyntaxKind::IndexExpression);
                parser.bump();
                expression(parser);
                parser.expect(SyntaxKind::RightBracket, "']'");
                ExpressionKind::Index
            }
            SyntaxKind::Colon => {
                parser.start_node_at(checkpoint, SyntaxKind::MethodCallExpression);
                parser.bump();
                parser.expect(SyntaxKind::Identifier, "a method name after ':'");
                if starts_type_instantiation(parser) {
                    types::type_instantiation(parser);
                }
                argument_list(parser);
                ExpressionKind::MethodCall
            }
            SyntaxKind::Less if starts_type_instantiation(parser) => {
                parser.start_node_at(checkpoint, SyntaxKind::InstantiationExpression);
                types::type_instantiation(parser);
                ExpressionKind::Instantiation
            }
            SyntaxKind::LeftParenthesis | SyntaxKind::LeftBrace | SyntaxKind::String | SyntaxKind::LongString => {
                parser.start_node_at(checkpoint, SyntaxKind::CallExpression);
                argument_list(parser);
                ExpressionKind::Call
            }
            _ => return Some(outermost),
        };
        parser.finish_node();
    }
}

/// `<<`, which no Lua 5.2 expression can continue with (there is no `<<`
/// operator, and `a < <b` is not an expression), so it never needs the
/// lookahead TypeScript's `f<T>(x)` would.
fn starts_type_instantiation(parser: &Parser) -> bool {
    parser.at(SyntaxKind::Less) && parser.nth(1) == SyntaxKind::Less
}

fn argument_list(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ArgumentList);
    match parser.current() {
        SyntaxKind::LeftParenthesis => {
            parser.bump();
            if !parser.at(SyntaxKind::RightParenthesis) {
                expression_list(parser);
            }
            parser.expect(SyntaxKind::RightParenthesis, "')'");
        }
        SyntaxKind::LeftBrace => table_constructor(parser),
        SyntaxKind::String | SyntaxKind::LongString => {
            parser.start_node(SyntaxKind::LiteralExpression);
            parser.bump();
            parser.finish_node();
        }
        _ => parser.error("expected arguments".to_string()),
    }
    parser.finish_node();
}

fn table_constructor(parser: &mut Parser) {
    parser.start_node(SyntaxKind::TableConstructor);
    parser.bump();
    while !matches!(parser.current(), SyntaxKind::RightBrace | SyntaxKind::EndOfFile) {
        table_field(parser);
        if !parser.eat(SyntaxKind::Comma) && !parser.eat(SyntaxKind::Semicolon) {
            break;
        }
    }
    parser.expect(SyntaxKind::RightBrace, "'}'");
    parser.finish_node();
}

fn table_field(parser: &mut Parser) {
    match parser.current() {
        SyntaxKind::LeftBracket => {
            parser.start_node(SyntaxKind::IndexedField);
            parser.bump();
            expression(parser);
            parser.expect(SyntaxKind::RightBracket, "']'");
            parser.expect(SyntaxKind::Equals, "'='");
            expression(parser);
        }
        SyntaxKind::Identifier if parser.nth(1) == SyntaxKind::Equals => {
            parser.start_node(SyntaxKind::NamedField);
            parser.bump();
            parser.bump();
            expression(parser);
        }
        _ => {
            parser.start_node(SyntaxKind::PositionalField);
            expression(parser);
        }
    }
    parser.finish_node();
}

/// The parameters and body shared by every function form:
/// `<T>(parameters): Type block end`.
pub(super) fn function_body(parser: &mut Parser) {
    parser.start_node(SyntaxKind::FunctionBody);
    types::optional_type_parameter_list(parser);
    parameter_list(parser, SyntaxKind::LeftParenthesis, SyntaxKind::RightParenthesis);
    types::optional_return_type_annotation(parser);
    statements::block_then_end(parser);
    parser.finish_node();
}

/// MoonSharp's short lambda, `|parameters| expression`.
fn lambda_expression(parser: &mut Parser) {
    parser.start_node(SyntaxKind::LambdaExpression);
    parameter_list(parser, SyntaxKind::Pipe, SyntaxKind::Pipe);
    expression(parser);
    parser.finish_node();
}

pub(super) fn parameter_list(parser: &mut Parser, open: SyntaxKind, close: SyntaxKind) {
    parser.start_node(SyntaxKind::ParameterList);
    let open_description = if open == SyntaxKind::Pipe { "'|'" } else { "'('" };
    parser.expect(open, open_description);
    if !parser.at(close) {
        loop {
            parameter(parser, close);
            if !parser.eat(SyntaxKind::Comma) {
                break;
            }
        }
    }
    let close_description = if close == SyntaxKind::Pipe { "'|'" } else { "')'" };
    parser.expect(close, close_description);
    parser.finish_node();
}

/// `close` is the token ending the parameter list; when it is `|`, an
/// annotation cannot contain a bare union.
fn parameter(parser: &mut Parser, close: SyntaxKind) {
    parser.start_node(SyntaxKind::Parameter);
    if !parser.eat(SyntaxKind::TripleDot) {
        parser.expect(SyntaxKind::Identifier, "a parameter name");
        if parser.at(SyntaxKind::Question) {
            parser.require_typed_dialect("optional parameters");
            parser.bump();
        }
    }
    if parser.at(SyntaxKind::Colon) {
        if close == SyntaxKind::Pipe {
            types::type_annotation_without_union(parser);
        } else {
            types::type_annotation(parser);
        }
    }
    parser.finish_node();
}
