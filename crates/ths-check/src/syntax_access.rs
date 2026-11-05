//! Small queries over the untyped syntax tree.

use ths_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|child| child.kind() == kind)
}

pub(crate) fn children(node: &SyntaxNode, kind: SyntaxKind) -> impl Iterator<Item = SyntaxNode> {
    node.children().filter(move |child| child.kind() == kind)
}

/// Tokens directly inside `node`, without trivia.
pub(crate) fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(SyntaxElement::into_token).filter(|token| !token.kind().is_trivia())
}

pub(crate) fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|token| token.kind() == kind)
}

pub(crate) fn first_name(node: &SyntaxNode) -> Option<SyntaxToken> {
    token(node, SyntaxKind::Identifier)
}

/// The name a `type`, `interface`, `declare name: T` or `declare type`
/// statement declares: the name after the contextual keywords, which are
/// themselves identifiers.
pub(crate) fn declared_name(statement: &SyntaxNode) -> Option<SyntaxToken> {
    let keywords = if statement.kind() == SyntaxKind::DeclareTypeStatement { 2 } else { 1 };
    tokens(statement).filter(|token| token.kind() == SyntaxKind::Identifier).nth(keywords)
}

pub(crate) fn is_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NameExpression
            | SyntaxKind::LiteralExpression
            | SyntaxKind::VarargExpression
            | SyntaxKind::FunctionExpression
            | SyntaxKind::LambdaExpression
            | SyntaxKind::TableConstructor
            | SyntaxKind::BinaryExpression
            | SyntaxKind::UnaryExpression
            | SyntaxKind::ParenthesizedExpression
            | SyntaxKind::FieldExpression
            | SyntaxKind::IndexExpression
            | SyntaxKind::CallExpression
            | SyntaxKind::MethodCallExpression
            | SyntaxKind::CastExpression
            | SyntaxKind::InstantiationExpression
    )
}

pub(crate) fn is_type(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NameType
            | SyntaxKind::LiteralType
            | SyntaxKind::ArrayType
            | SyntaxKind::UnionType
            | SyntaxKind::IntersectionType
            | SyntaxKind::ParenthesizedType
            | SyntaxKind::FunctionType
            | SyntaxKind::ObjectType
            | SyntaxKind::TypeList
            | SyntaxKind::VariadicType
    )
}

pub(crate) fn expressions(node: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    node.children().filter(|child| is_expression(child.kind()))
}

pub(crate) fn first_expression(node: &SyntaxNode) -> Option<SyntaxNode> {
    expressions(node).next()
}

pub(crate) fn first_type(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children().find(|child| is_type(child.kind()))
}

/// The type inside a `TypeAnnotation` or `ReturnTypeAnnotation` child.
pub(crate) fn annotation_type(node: &SyntaxNode, annotation_kind: SyntaxKind) -> Option<SyntaxNode> {
    child(node, annotation_kind).and_then(|annotation| first_type(&annotation))
}

/// The argument expressions: `f(a, b)`, `f{...}` or `f"text"`.
pub(crate) fn call_arguments(call: &SyntaxNode) -> Vec<SyntaxNode> {
    let Some(list) = child(call, SyntaxKind::ArgumentList) else {
        return Vec::new();
    };
    match child(&list, SyntaxKind::ExpressionList) {
        Some(expressions_list) => expressions(&expressions_list).collect(),
        None => list.children().filter(|node| is_expression(node.kind())).collect(),
    }
}
