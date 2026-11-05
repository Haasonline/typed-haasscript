//! Type grammar for the typed dialect, following TypeScript: annotations,
//! type parameters and arguments, and the type expressions themselves.

use super::Parser;
use super::contextual_keyword::ContextualKeyword;
use super::expressions;
use crate::syntax_kind::SyntaxKind;

/// `: Type` after a name or parameter. The caller has checked for `:`.
pub(super) fn type_annotation(parser: &mut Parser) {
    annotation(parser, SyntaxKind::TypeAnnotation);
}

/// `: Type` after a function's parameter list, if present.
pub(super) fn optional_return_type_annotation(parser: &mut Parser) {
    if parser.at(SyntaxKind::Colon) {
        annotation(parser, SyntaxKind::ReturnTypeAnnotation);
    }
}

/// `: Type` on a lambda parameter, where `|` closes the parameter list, so a
/// union must be parenthesized: `|x: (A | B)| ...`.
pub(super) fn type_annotation_without_union(parser: &mut Parser) {
    parser.start_node(SyntaxKind::TypeAnnotation);
    parser.require_typed_dialect("type annotations");
    parser.bump();
    intersection_type(parser);
    parser.finish_node();
}

fn annotation(parser: &mut Parser, kind: SyntaxKind) {
    parser.start_node(kind);
    parser.require_typed_dialect("type annotations");
    parser.bump();
    type_expression(parser);
    parser.finish_node();
}

/// `<T, U extends V = W>`, if present.
pub(super) fn optional_type_parameter_list(parser: &mut Parser) {
    if !parser.at(SyntaxKind::Less) {
        return;
    }
    parser.start_node(SyntaxKind::TypeParameterList);
    parser.require_typed_dialect("type parameters");
    parser.bump();
    loop {
        parser.start_node(SyntaxKind::TypeParameter);
        parser.expect(SyntaxKind::Identifier, "a type parameter name");
        if parser.nth_is_keyword(0, ContextualKeyword::Extends) {
            parser.bump();
            type_expression(parser);
        }
        if parser.eat(SyntaxKind::Equals) {
            type_expression(parser);
        }
        parser.finish_node();
        if !parser.eat(SyntaxKind::Comma) {
            break;
        }
    }
    close_angle_bracket(parser);
    parser.finish_node();
}

fn type_argument_list(parser: &mut Parser) {
    parser.start_node(SyntaxKind::TypeArgumentList);
    parser.bump();
    loop {
        type_expression(parser);
        if !parser.eat(SyntaxKind::Comma) {
            break;
        }
    }
    close_angle_bracket(parser);
    parser.finish_node();
}

/// `<<T, U>>`: explicit type arguments, Luau's syntax for them
/// (`f<<number>>(x)`). The caller has checked for `<<`.
pub(super) fn type_instantiation(parser: &mut Parser) {
    parser.start_node(SyntaxKind::TypeInstantiation);
    parser.require_typed_dialect("type arguments");
    parser.bump();
    parser.bump();
    loop {
        type_expression(parser);
        if !parser.eat(SyntaxKind::Comma) {
            break;
        }
    }
    close_angle_bracket(parser);
    close_angle_bracket(parser);
    parser.finish_node();
}

/// Consumes the `>` closing a type parameter or argument list. `>=` is split
/// first, as in `local x: Array<number>= {}`.
fn close_angle_bracket(parser: &mut Parser) {
    if parser.at(SyntaxKind::GreaterEquals) {
        parser.split_current_token(SyntaxKind::Greater, SyntaxKind::Equals);
    }
    parser.expect(SyntaxKind::Greater, "'>'");
}

pub(super) fn type_expression(parser: &mut Parser) {
    union_type(parser);
}

fn union_type(parser: &mut Parser) {
    separated_type(parser, SyntaxKind::Pipe, SyntaxKind::UnionType, intersection_type);
}

fn intersection_type(parser: &mut Parser) {
    separated_type(parser, SyntaxKind::Ampersand, SyntaxKind::IntersectionType, array_type);
}

/// Operands joined by `separator` (`A | B | C`) as one `kind` node, with an
/// optional leading separator as TypeScript allows (`| "long" | "short"`).
/// A single operand gets no node.
fn separated_type(parser: &mut Parser, separator: SyntaxKind, kind: SyntaxKind, operand: fn(&mut Parser)) {
    let checkpoint = parser.checkpoint();
    let has_leading_separator = parser.eat(separator);
    operand(parser);
    if !has_leading_separator && !parser.at(separator) {
        return;
    }
    parser.start_node_at(checkpoint, kind);
    while parser.eat(separator) {
        operand(parser);
    }
    parser.finish_node();
}

/// A primary type followed by any number of `[]`.
fn array_type(parser: &mut Parser) {
    let checkpoint = parser.checkpoint();
    primary_type(parser);
    while parser.at(SyntaxKind::LeftBracket) && parser.nth(1) == SyntaxKind::RightBracket {
        parser.start_node_at(checkpoint, SyntaxKind::ArrayType);
        parser.bump();
        parser.bump();
        parser.finish_node();
    }
}

fn primary_type(parser: &mut Parser) {
    match parser.current() {
        SyntaxKind::Identifier => {
            parser.start_node(SyntaxKind::NameType);
            parser.bump();
            if parser.at(SyntaxKind::Less) {
                type_argument_list(parser);
            }
            parser.finish_node();
        }
        SyntaxKind::String
        | SyntaxKind::Number
        | SyntaxKind::NilKeyword
        | SyntaxKind::TrueKeyword
        | SyntaxKind::FalseKeyword => {
            parser.start_node(SyntaxKind::LiteralType);
            parser.bump();
            parser.finish_node();
        }
        SyntaxKind::Less => function_type(parser),
        SyntaxKind::LeftParenthesis if parser.parenthesis_is_followed_by(SyntaxKind::FatArrow) => {
            function_type(parser);
        }
        SyntaxKind::LeftParenthesis => parenthesized_type(parser),
        SyntaxKind::LeftBrace => object_type(parser),
        SyntaxKind::TripleDot => {
            parser.start_node(SyntaxKind::VariadicType);
            parser.bump();
            type_expression(parser);
            parser.finish_node();
        }
        _ => parser.error("expected a type".to_string()),
    }
}

/// `(A)`, or a list of several values: `(A, B)`, `(A, ...B)` or `(...B)`.
/// The list is parsed in any type position and the checker reports it
/// outside a return type, so a misplaced list gives one clear error instead
/// of a cascade.
fn parenthesized_type(parser: &mut Parser) {
    let checkpoint = parser.checkpoint();
    parser.bump();
    let starts_variadic = parser.at(SyntaxKind::TripleDot);
    type_expression(parser);
    let kind = if starts_variadic || parser.at(SyntaxKind::Comma) {
        SyntaxKind::TypeList
    } else {
        SyntaxKind::ParenthesizedType
    };
    parser.start_node_at(checkpoint, kind);
    while parser.eat(SyntaxKind::Comma) {
        type_expression(parser);
    }
    parser.expect(SyntaxKind::RightParenthesis, "')'");
    parser.finish_node();
}

/// `<T>(a: T, b?: number, ...: string) => T`
fn function_type(parser: &mut Parser) {
    parser.start_node(SyntaxKind::FunctionType);
    optional_type_parameter_list(parser);
    expressions::parameter_list(parser, SyntaxKind::LeftParenthesis, SyntaxKind::RightParenthesis);
    parser.expect(SyntaxKind::FatArrow, "'=>'");
    type_expression(parser);
    parser.finish_node();
}

/// `{ name: T, optional?: T; 1: T; method(a: T): R  [key: string]: T  (a: T): R }`:
/// members separated by `,`, `;` or nothing, as in a TypeScript interface
/// body. A numeric name types one index, as in TypeScript.
pub(super) fn object_type(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ObjectType);
    parser.expect(SyntaxKind::LeftBrace, "'{'");
    while !matches!(parser.current(), SyntaxKind::RightBrace | SyntaxKind::EndOfFile) {
        match parser.current() {
            SyntaxKind::Identifier | SyntaxKind::Number => named_member(parser),
            SyntaxKind::LeftBracket => index_signature(parser),
            SyntaxKind::LeftParenthesis | SyntaxKind::Less => call_signature(parser),
            _ => {
                parser.error_and_bump("expected a member name, '[' or '('".to_string());
                continue;
            }
        }
        if !parser.eat(SyntaxKind::Comma) {
            parser.eat(SyntaxKind::Semicolon);
        }
    }
    parser.expect(SyntaxKind::RightBrace, "'}'");
    parser.finish_node();
}

fn named_member(parser: &mut Parser) {
    let checkpoint = parser.checkpoint();
    parser.bump();
    parser.eat(SyntaxKind::Question);
    if matches!(parser.current(), SyntaxKind::LeftParenthesis | SyntaxKind::Less) {
        parser.start_node_at(checkpoint, SyntaxKind::MethodSignature);
        optional_type_parameter_list(parser);
        expressions::parameter_list(parser, SyntaxKind::LeftParenthesis, SyntaxKind::RightParenthesis);
        optional_return_type_annotation(parser);
    } else {
        parser.start_node_at(checkpoint, SyntaxKind::PropertySignature);
        if parser.at(SyntaxKind::Colon) {
            type_annotation(parser);
        } else {
            parser.error("expected ':'".to_string());
        }
    }
    parser.finish_node();
}

/// `<T>(a: T): R`
fn call_signature(parser: &mut Parser) {
    parser.start_node(SyntaxKind::CallSignature);
    optional_type_parameter_list(parser);
    expressions::parameter_list(parser, SyntaxKind::LeftParenthesis, SyntaxKind::RightParenthesis);
    optional_return_type_annotation(parser);
    parser.finish_node();
}

/// `[key: string]: T`
fn index_signature(parser: &mut Parser) {
    parser.start_node(SyntaxKind::IndexSignature);
    parser.bump();
    parser.expect(SyntaxKind::Identifier, "a key name");
    if parser.at(SyntaxKind::Colon) {
        type_annotation(parser);
    } else {
        parser.error("expected ':'".to_string());
    }
    parser.expect(SyntaxKind::RightBracket, "']'");
    if parser.at(SyntaxKind::Colon) {
        type_annotation(parser);
    } else {
        parser.error("expected ':'".to_string());
    }
    parser.finish_node();
}
