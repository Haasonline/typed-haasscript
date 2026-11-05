//! Statement grammar: Lua 5.2 statements as MoonSharp accepts them.

use rowan::Checkpoint;

use super::Parser;
use super::contextual_keyword::ContextualKeyword;
use super::expressions::{self, ExpressionKind};
use super::modules;
use super::types;
use crate::syntax_kind::SyntaxKind;

pub(super) fn chunk(parser: &mut Parser) {
    block(parser);
    if !parser.at(SyntaxKind::EndOfFile) {
        // A stray `end`, `until`, ... that closes nothing.
        while !parser.at(SyntaxKind::EndOfFile) {
            parser.error_and_bump("unexpected token outside any block".to_string());
            block(parser);
        }
    }
}

fn block(parser: &mut Parser) {
    parser.start_node(SyntaxKind::Block);
    loop {
        match parser.current() {
            SyntaxKind::EndOfFile
            | SyntaxKind::EndKeyword
            | SyntaxKind::ElseKeyword
            | SyntaxKind::ElseifKeyword
            | SyntaxKind::UntilKeyword => break,
            SyntaxKind::ReturnKeyword => {
                return_statement(parser);
                break;
            }
            _ => statement(parser),
        }
    }
    parser.finish_node();
}

fn statement(parser: &mut Parser) {
    match parser.current() {
        SyntaxKind::Semicolon => single_token_statement(parser, SyntaxKind::EmptyStatement),
        SyntaxKind::BreakKeyword => single_token_statement(parser, SyntaxKind::BreakStatement),
        SyntaxKind::IfKeyword => if_statement(parser),
        SyntaxKind::WhileKeyword => while_statement(parser),
        SyntaxKind::DoKeyword => do_statement(parser),
        SyntaxKind::ForKeyword => for_statement(parser),
        SyntaxKind::RepeatKeyword => repeat_statement(parser),
        SyntaxKind::FunctionKeyword => function_statement(parser),
        SyntaxKind::LocalKeyword => local_statement(parser),
        SyntaxKind::GotoKeyword => goto_statement(parser),
        SyntaxKind::DoubleColon => label_statement(parser),
        _ if parser.is_typed() => typed_declaration_or_expression_statement(parser),
        _ => expression_statement(parser),
    }
}

fn typed_declaration_or_expression_statement(parser: &mut Parser) {
    let next = parser.nth(1);
    if modules::at_import(parser) {
        modules::import_statement(parser);
    } else if modules::at_export(parser) {
        modules::export_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Type) && next == SyntaxKind::Identifier {
        type_alias_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Interface) && next == SyntaxKind::Identifier {
        interface_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Declare)
        && parser.nth_is_keyword(1, ContextualKeyword::Type)
        && parser.nth(2) == SyntaxKind::Identifier
    {
        declare_type_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Declare) && next == SyntaxKind::FunctionKeyword {
        declare_function_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Declare) && next == SyntaxKind::Identifier {
        declare_variable_statement(parser);
    } else {
        expression_statement(parser);
    }
}

/// `type Name<T> = Type`
pub(super) fn type_alias_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::TypeAliasStatement);
    parser.bump();
    parser.bump();
    types::optional_type_parameter_list(parser);
    parser.expect(SyntaxKind::Equals, "'='");
    types::type_expression(parser);
    parser.finish_node();
}

/// `interface Name<T> extends A, B { members }`
pub(super) fn interface_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::InterfaceStatement);
    parser.bump();
    parser.bump();
    types::optional_type_parameter_list(parser);
    optional_extends_clause(parser);
    types::object_type(parser);
    parser.finish_node();
}

/// `declare type Name extends A, B { (a: T): R }`: a type known only by its
/// name, such as an engine value, optionally callable. `declare type: T`
/// stays a variable named `type`.
fn declare_type_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::DeclareTypeStatement);
    parser.bump();
    parser.bump();
    parser.bump();
    optional_extends_clause(parser);
    if parser.at(SyntaxKind::LeftBrace) {
        types::object_type(parser);
    }
    parser.finish_node();
}

/// `extends A, B`, if present.
fn optional_extends_clause(parser: &mut Parser) {
    if !parser.nth_is_keyword(0, ContextualKeyword::Extends) {
        return;
    }
    parser.start_node(SyntaxKind::ExtendsClause);
    parser.bump();
    loop {
        types::type_expression(parser);
        if !parser.eat(SyntaxKind::Comma) {
            break;
        }
    }
    parser.finish_node();
}

/// `declare function name.path<T>(parameters): Type`, with no body.
fn declare_function_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::DeclareFunctionStatement);
    parser.bump();
    parser.bump();
    function_name(parser);
    types::optional_type_parameter_list(parser);
    expressions::parameter_list(parser, SyntaxKind::LeftParenthesis, SyntaxKind::RightParenthesis);
    types::optional_return_type_annotation(parser);
    parser.finish_node();
}

/// `declare name: Type`
fn declare_variable_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::DeclareVariableStatement);
    parser.bump();
    parser.bump();
    if parser.at(SyntaxKind::Colon) {
        types::type_annotation(parser);
    } else {
        parser.error("expected ':'".to_string());
    }
    parser.finish_node();
}

fn single_token_statement(parser: &mut Parser, kind: SyntaxKind) {
    parser.start_node(kind);
    parser.bump();
    parser.finish_node();
}

/// `... end`, closing a statement or function that opened a block.
pub(super) fn block_then_end(parser: &mut Parser) {
    block(parser);
    parser.expect(SyntaxKind::EndKeyword, "'end'");
}

fn if_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::IfStatement);
    parser.bump();
    expressions::expression(parser);
    parser.expect(SyntaxKind::ThenKeyword, "'then'");
    block(parser);
    while parser.at(SyntaxKind::ElseifKeyword) {
        parser.start_node(SyntaxKind::ElseifClause);
        parser.bump();
        expressions::expression(parser);
        parser.expect(SyntaxKind::ThenKeyword, "'then'");
        block(parser);
        parser.finish_node();
    }
    if parser.at(SyntaxKind::ElseKeyword) {
        parser.start_node(SyntaxKind::ElseClause);
        parser.bump();
        block(parser);
        parser.finish_node();
    }
    parser.expect(SyntaxKind::EndKeyword, "'end'");
    parser.finish_node();
}

fn while_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::WhileStatement);
    parser.bump();
    expressions::expression(parser);
    parser.expect(SyntaxKind::DoKeyword, "'do'");
    block_then_end(parser);
    parser.finish_node();
}

fn do_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::DoStatement);
    parser.bump();
    block_then_end(parser);
    parser.finish_node();
}

fn repeat_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::RepeatStatement);
    parser.bump();
    block(parser);
    parser.expect(SyntaxKind::UntilKeyword, "'until'");
    expressions::expression(parser);
    parser.finish_node();
}

/// The first name decides the form: `for i = ...` is numeric, anything else
/// generic. The name may carry a type annotation, so look past it.
fn for_statement(parser: &mut Parser) {
    let statement_checkpoint = parser.checkpoint();
    parser.bump();
    let names_checkpoint = parser.checkpoint();
    name_binding(parser);
    if parser.at(SyntaxKind::Equals) {
        parser.start_node_at(statement_checkpoint, SyntaxKind::NumericForStatement);
        parser.bump();
        expressions::expression(parser);
        parser.expect(SyntaxKind::Comma, "','");
        expressions::expression(parser);
        if parser.eat(SyntaxKind::Comma) {
            expressions::expression(parser);
        }
    } else {
        parser.start_node_at(statement_checkpoint, SyntaxKind::GenericForStatement);
        parser.start_node_at(names_checkpoint, SyntaxKind::NameList);
        while parser.eat(SyntaxKind::Comma) {
            name_binding(parser);
        }
        parser.finish_node();
        parser.expect(SyntaxKind::InKeyword, "'=' or 'in'");
        expressions::expression_list(parser);
    }
    parser.expect(SyntaxKind::DoKeyword, "'do'");
    block_then_end(parser);
    parser.finish_node();
}

fn function_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::FunctionStatement);
    parser.bump();
    function_name(parser);
    expressions::function_body(parser);
    parser.finish_node();
}

/// `a.b.c` or `a.b:c`
fn function_name(parser: &mut Parser) {
    parser.start_node(SyntaxKind::FunctionName);
    parser.expect(SyntaxKind::Identifier, "a function name");
    while parser.eat(SyntaxKind::Dot) {
        parser.expect(SyntaxKind::Identifier, "a name after '.'");
    }
    if parser.eat(SyntaxKind::Colon) {
        parser.expect(SyntaxKind::Identifier, "a method name after ':'");
    }
    parser.finish_node();
}

/// `local function name ...` or `local names = values`.
pub(super) fn local_statement(parser: &mut Parser) {
    if parser.nth(1) == SyntaxKind::FunctionKeyword {
        local_function_statement(parser);
    } else {
        local_variable_statement(parser);
    }
}

/// `local function name(parameters) ... end`
fn local_function_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::LocalFunctionStatement);
    parser.bump();
    parser.bump();
    parser.expect(SyntaxKind::Identifier, "a function name");
    expressions::function_body(parser);
    parser.finish_node();
}

/// `local a, b = 1, 2`, the values optional.
fn local_variable_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::LocalStatement);
    parser.bump();
    name_list(parser);
    if parser.eat(SyntaxKind::Equals) {
        expressions::expression_list(parser);
    }
    parser.finish_node();
}

fn name_list(parser: &mut Parser) {
    parser.start_node(SyntaxKind::NameList);
    name_binding(parser);
    while parser.eat(SyntaxKind::Comma) {
        name_binding(parser);
    }
    parser.finish_node();
}

fn name_binding(parser: &mut Parser) {
    parser.start_node(SyntaxKind::NameBinding);
    parser.expect(SyntaxKind::Identifier, "a name");
    if parser.at(SyntaxKind::Colon) {
        types::type_annotation(parser);
    }
    parser.finish_node();
}

fn return_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ReturnStatement);
    parser.bump();
    if expressions::can_start_expression(parser.current()) {
        expressions::expression_list(parser);
    }
    parser.eat(SyntaxKind::Semicolon);
    parser.finish_node();
}

fn goto_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::GotoStatement);
    parser.bump();
    parser.expect(SyntaxKind::Identifier, "a label name");
    parser.finish_node();
}

fn label_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::LabelStatement);
    parser.bump();
    parser.expect(SyntaxKind::Identifier, "a label name");
    parser.expect(SyntaxKind::DoubleColon, "'::'");
    parser.finish_node();
}

/// A function call, or an assignment to one or more targets.
fn expression_statement(parser: &mut Parser) {
    if !expressions::can_start_suffixed_expression(parser.current()) {
        parser.error_and_bump("expected a statement".to_string());
        return;
    }
    let checkpoint = parser.checkpoint();
    let first = expressions::suffixed_expression(parser);
    if matches!(parser.current(), SyntaxKind::Equals | SyntaxKind::Comma) {
        assignment_statement_rest(parser, checkpoint, first);
        return;
    }
    if matches!(first, Some(ExpressionKind::Call | ExpressionKind::MethodCall)) {
        parser.start_node_at(checkpoint, SyntaxKind::CallStatement);
        parser.finish_node();
        return;
    }
    parser.start_node_at(checkpoint, SyntaxKind::ErrorNode);
    parser.error("expected '=' or a function call".to_string());
    parser.finish_node();
}

/// `, b = 1, 2` after the first target, which has already been parsed from
/// `checkpoint` as `first`.
fn assignment_statement_rest(parser: &mut Parser, checkpoint: Checkpoint, first: Option<ExpressionKind>) {
    parser.start_node_at(checkpoint, SyntaxKind::AssignmentStatement);
    require_assignable(parser, first);
    while parser.eat(SyntaxKind::Comma) {
        let target = expressions::suffixed_expression(parser);
        require_assignable(parser, target);
    }
    parser.expect(SyntaxKind::Equals, "'='");
    expressions::expression_list(parser);
    parser.finish_node();
}

fn require_assignable(parser: &mut Parser, target: Option<ExpressionKind>) {
    let is_assignable = matches!(target, Some(ExpressionKind::Name | ExpressionKind::Field | ExpressionKind::Index));
    if !is_assignable {
        parser.error("cannot assign to this expression".to_string());
    }
}
