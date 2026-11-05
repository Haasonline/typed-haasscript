//! Where a declared name can be read, by Lua's scoping: a `local` from the
//! end of its statement to the end of its block, a `local function` from
//! its name (so it can recurse), a parameter in its function, and a `for`
//! variable in the loop's body.

use rowan::{TextRange, TextSize};
use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::syntax_access::child;

/// What kind of declaration a name is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingSite {
    Local,
    LocalFunction,
    Parameter,
    LoopVariable,
}

/// The range in which the name declared at `name` can be read; `None`
/// when the syntax around it is incomplete.
pub(crate) fn visible_range(name: &SyntaxToken, site: BindingSite) -> Option<TextRange> {
    match site {
        BindingSite::Local => {
            let statement = ancestor(name, SyntaxKind::LocalStatement)?;
            let block = enclosing_block(&statement)?;
            Some(TextRange::new(statement.text_range().end(), scope_end(&block)))
        }
        BindingSite::LocalFunction => {
            let statement = ancestor(name, SyntaxKind::LocalFunctionStatement)?;
            let block = enclosing_block(&statement)?;
            Some(TextRange::new(name.text_range().start(), scope_end(&block)))
        }
        BindingSite::Parameter => name
            .parent_ancestors()
            .find(|node| matches!(node.kind(), SyntaxKind::FunctionBody | SyntaxKind::LambdaExpression))
            .map(|function| function.text_range()),
        BindingSite::LoopVariable => {
            let statement = name.parent_ancestors().find(|node| {
                matches!(node.kind(), SyntaxKind::NumericForStatement | SyntaxKind::GenericForStatement)
            })?;
            let body = child(&statement, SyntaxKind::Block)?;
            Some(TextRange::new(body.text_range().start(), scope_end(&body)))
        }
    }
}

fn ancestor(name: &SyntaxToken, kind: SyntaxKind) -> Option<SyntaxNode> {
    name.parent_ancestors().find(|node| node.kind() == kind)
}

fn enclosing_block(statement: &SyntaxNode) -> Option<SyntaxNode> {
    statement.ancestors().skip(1).find(|node| node.kind() == SyntaxKind::Block)
}

/// Where the scope of `block` ends: at the keyword that closes it (`end`,
/// `else`, `elseif`, `until`), or the end of the file. A block's node ends
/// with its last statement, so the lines after that are found this way.
fn scope_end(block: &SyntaxNode) -> TextSize {
    let mut token = block.last_token().and_then(|last| last.next_token());
    while let Some(current) = token {
        if !current.kind().is_trivia() {
            return current.text_range().start();
        }
        token = current.next_token();
    }
    block.ancestors().last().map_or(block.text_range().end(), |root| root.text_range().end())
}
