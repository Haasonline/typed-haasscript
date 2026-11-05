use std::fmt::Write;

use crate::{SyntaxElement, SyntaxNode};

/// The tree as indented lines of node kinds and token texts, without
/// trivia, so tests read like the grammar.
pub(crate) fn outline(node: &SyntaxNode) -> String {
    let mut text = String::new();
    write_outline(node, 0, &mut text);
    text
}

fn write_outline(node: &SyntaxNode, depth: usize, text: &mut String) {
    writeln!(text, "{}{:?}", "  ".repeat(depth), node.kind()).unwrap();
    for child in node.children_with_tokens() {
        match child {
            SyntaxElement::Node(child) => write_outline(&child, depth + 1, text),
            SyntaxElement::Token(token) if !token.kind().is_trivia() => {
                writeln!(text, "{}{:?}", "  ".repeat(depth + 1), token.text()).unwrap();
            }
            SyntaxElement::Token(_) => {}
        }
    }
}
