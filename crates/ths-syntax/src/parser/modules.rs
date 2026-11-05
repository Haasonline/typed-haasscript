//! Module grammar, typed dialect only: TypeScript's `import` and `export`,
//! named only.
//!
//! ```text
//! import { average, sum as total, type Prices } from "./math/averages"
//! import * as averages from "./math/averages"
//! export local function average(...) ... end
//! export type Prices = number[]
//! export { helper as assist }
//! ```

use super::Parser;
use super::contextual_keyword::ContextualKeyword;
use super::statements;
use crate::syntax_kind::SyntaxKind;

/// Whether the statement starting here is an import.
pub(super) fn at_import(parser: &Parser) -> bool {
    parser.nth_is_keyword(0, ContextualKeyword::Import)
        && matches!(parser.nth(1), SyntaxKind::LeftBrace | SyntaxKind::Star)
}

/// Whether the statement starting here is an export.
pub(super) fn at_export(parser: &Parser) -> bool {
    if !parser.nth_is_keyword(0, ContextualKeyword::Export) {
        return false;
    }
    let declares_type = (parser.nth_is_keyword(1, ContextualKeyword::Type)
        || parser.nth_is_keyword(1, ContextualKeyword::Interface))
        && parser.nth(2) == SyntaxKind::Identifier;
    matches!(parser.nth(1), SyntaxKind::LocalKeyword | SyntaxKind::LeftBrace) || declares_type
}

/// `import { a, b as c, type T } from "path"` or `import * as m from "path"`.
pub(super) fn import_statement(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ImportStatement);
    parser.bump();
    if parser.at(SyntaxKind::Star) {
        parser.start_node(SyntaxKind::NamespaceImport);
        parser.bump();
        expect_keyword(parser, ContextualKeyword::As);
        parser.expect(SyntaxKind::Identifier, "a name");
        parser.finish_node();
    } else {
        specifier_list(parser, import_specifier);
    }
    expect_keyword(parser, ContextualKeyword::From);
    parser.expect(SyntaxKind::String, "a module path");
    parser.finish_node();
}

/// `export` and a `local`, `local function`, `type` or `interface`
/// declaration, or `export { a, b as c }`.
pub(super) fn export_statement(parser: &mut Parser) {
    if parser.nth(1) == SyntaxKind::LeftBrace {
        parser.start_node(SyntaxKind::ExportListStatement);
        parser.bump();
        specifier_list(parser, export_specifier);
        parser.finish_node();
        return;
    }
    parser.start_node(SyntaxKind::ExportDeclaration);
    parser.bump();
    if parser.at(SyntaxKind::LocalKeyword) {
        statements::local_statement(parser);
    } else if parser.nth_is_keyword(0, ContextualKeyword::Type) {
        statements::type_alias_statement(parser);
    } else {
        statements::interface_statement(parser);
    }
    parser.finish_node();
}

/// `{ a, b }`, a trailing comma allowed.
fn specifier_list(parser: &mut Parser, specifier: fn(&mut Parser)) {
    parser.expect(SyntaxKind::LeftBrace, "'{'");
    while parser.at(SyntaxKind::Identifier) {
        specifier(parser);
        if !parser.eat(SyntaxKind::Comma) {
            break;
        }
    }
    parser.expect(SyntaxKind::RightBrace, "'}'");
}

/// `type T`, `a` or `b as c`. `type` marks a type-only import when a name
/// other than `as` follows it.
fn import_specifier(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ImportSpecifier);
    if parser.nth_is_keyword(0, ContextualKeyword::Type)
        && parser.nth(1) == SyntaxKind::Identifier
        && !parser.nth_is_keyword(1, ContextualKeyword::As)
    {
        parser.bump();
    }
    name_and_alias(parser);
    parser.finish_node();
}

/// `a` or `b as c`.
fn export_specifier(parser: &mut Parser) {
    parser.start_node(SyntaxKind::ExportSpecifier);
    name_and_alias(parser);
    parser.finish_node();
}

fn name_and_alias(parser: &mut Parser) {
    parser.expect(SyntaxKind::Identifier, "a name");
    if parser.nth_is_keyword(0, ContextualKeyword::As) {
        parser.bump();
        parser.expect(SyntaxKind::Identifier, "a name");
    }
}

/// The contextual keyword `keyword`, reported as expected when absent.
fn expect_keyword(parser: &mut Parser, keyword: ContextualKeyword) {
    if parser.nth_is_keyword(0, keyword) {
        parser.bump();
    } else {
        parser.error(format!("expected '{}'", keyword.text()));
    }
}
