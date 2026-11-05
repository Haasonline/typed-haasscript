//! Finds the parts of a typed syntax tree that plain HaasScript does not
//! have.

use rowan::{TextRange, WalkEvent};
use ths_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

/// A range to blank out, and whether removing it can leave two statements
/// with nothing between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Erasure {
    pub(crate) range: TextRange,
    pub(crate) ends_a_statement: bool,
}

/// Every range to blank out, in source order, without overlaps.
pub(crate) fn erasures(root: &SyntaxNode) -> Vec<Erasure> {
    let mut found = Vec::new();
    let mut preorder = root.preorder();
    while let Some(event) = preorder.next() {
        let WalkEvent::Enter(node) = event else {
            continue;
        };
        if let Some(erasure) = erasure_for(&node) {
            found.push(erasure);
            if erasure.range == node.text_range() {
                // Nothing inside a removed node needs a range of its own.
                preorder.skip_subtree();
            }
        }
    }
    without_nested(found)
}

/// Sorts by position and drops ranges inside an earlier one, such as an
/// annotation within a cast's type (`x as (a: number) => T`). Nested casts
/// (`a as T as U`) are found outer first, hence the sort.
fn without_nested(mut found: Vec<Erasure>) -> Vec<Erasure> {
    found.sort_by_key(|erasure| erasure.range.start());
    let mut kept: Vec<Erasure> = Vec::with_capacity(found.len());
    for erasure in found {
        let is_nested = kept.last().is_some_and(|previous| previous.range.contains_range(erasure.range));
        if !is_nested {
            kept.push(erasure);
        }
    }
    kept
}

fn erasure_for(node: &SyntaxNode) -> Option<Erasure> {
    let whole_node = |ends_a_statement| Some(Erasure { range: node.text_range(), ends_a_statement });
    match node.kind() {
        SyntaxKind::TypeAliasStatement
        | SyntaxKind::InterfaceStatement
        | SyntaxKind::DeclareFunctionStatement
        | SyntaxKind::DeclareVariableStatement
        | SyntaxKind::DeclareTypeStatement
        // The bundler writes what an import or an export list means.
        | SyntaxKind::ImportStatement
        | SyntaxKind::ExportListStatement => whole_node(true),
        SyntaxKind::TypeAnnotation
        | SyntaxKind::ReturnTypeAnnotation
        | SyntaxKind::TypeParameterList
        | SyntaxKind::TypeInstantiation => whole_node(false),
        SyntaxKind::CastExpression => cast_tail(node),
        SyntaxKind::Parameter => optional_marker(node),
        SyntaxKind::ExportDeclaration => export_keyword(node),
        _ => None,
    }
}

/// `as Type`, from the `as` to the end of the cast. The expression before it
/// stays; the cast may be the last thing in its statement.
fn cast_tail(node: &SyntaxNode) -> Option<Erasure> {
    let keyword = direct_tokens(node).find(|token| token.kind() == SyntaxKind::Identifier)?;
    Some(Erasure {
        range: TextRange::new(keyword.text_range().start(), node.text_range().end()),
        ends_a_statement: true,
    })
}

/// The `export` before an exported declaration, and the spaces after it;
/// the declaration stays.
fn export_keyword(node: &SyntaxNode) -> Option<Erasure> {
    let keyword = direct_tokens(node).find(|token| token.kind() == SyntaxKind::Identifier)?;
    let end = match keyword.next_token() {
        Some(space) if space.kind() == SyntaxKind::Whitespace => space.text_range().end(),
        _ => keyword.text_range().end(),
    };
    Some(Erasure { range: TextRange::new(keyword.text_range().start(), end), ends_a_statement: false })
}

/// The `?` of an optional parameter.
fn optional_marker(node: &SyntaxNode) -> Option<Erasure> {
    let marker = direct_tokens(node).find(|token| token.kind() == SyntaxKind::Question)?;
    Some(Erasure { range: marker.text_range(), ends_a_statement: false })
}

fn direct_tokens(node: &SyntaxNode) -> impl Iterator<Item = ths_syntax::SyntaxToken> {
    node.children_with_tokens().filter_map(SyntaxElement::into_token)
}
