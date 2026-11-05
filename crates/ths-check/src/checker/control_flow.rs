//! Syntactic facts about control flow that the checker needs: whether a
//! block always leaves early, and which variables a statement may assign.

use std::collections::HashSet;

use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::syntax_access::{child, children, expressions, first_name, tokens};

/// True when control never reaches the end of `block`: its last statement
/// is `return`, `break` or `goto`, or an `if` whose every branch, including
/// an `else`, always exits, or a `do` block that always exits.
pub(super) fn block_always_exits(block: &SyntaxNode) -> bool {
    let Some(last) = block.children().last() else {
        return false;
    };
    match last.kind() {
        SyntaxKind::ReturnStatement | SyntaxKind::BreakStatement | SyntaxKind::GotoStatement => true,
        SyntaxKind::DoStatement => child(&last, SyntaxKind::Block).is_some_and(|inner| block_always_exits(&inner)),
        SyntaxKind::IfStatement => if_always_exits(&last),
        _ => false,
    }
}

fn if_always_exits(statement: &SyntaxNode) -> bool {
    let Some(else_clause) = child(statement, SyntaxKind::ElseClause) else {
        return false;
    };
    let branch_blocks = child(statement, SyntaxKind::Block)
        .into_iter()
        .chain(children(statement, SyntaxKind::ElseifClause).filter_map(|clause| child(&clause, SyntaxKind::Block)))
        .chain(child(&else_clause, SyntaxKind::Block));
    branch_blocks.into_iter().all(|block| block_always_exits(&block))
}

/// Names that `node` may assign to anywhere inside it, nested functions
/// included, so narrowings of those names cannot be trusted across it.
pub(super) fn assigned_names(node: &SyntaxNode) -> HashSet<String> {
    let mut names = HashSet::new();
    for descendant in node.descendants() {
        match descendant.kind() {
            SyntaxKind::AssignmentStatement => {
                for target in assignment_targets(&descendant) {
                    if let Some(name) = root_variable(&target).and_then(|root| first_name(&root)) {
                        names.insert(name.text().to_string());
                    }
                }
            }
            SyntaxKind::FunctionStatement => {
                let simple_name = child(&descendant, SyntaxKind::FunctionName)
                    .filter(|name| tokens(name).count() == 1)
                    .and_then(|name| first_name(&name));
                if let Some(name) = simple_name {
                    names.insert(name.text().to_string());
                }
            }
            _ => {}
        }
    }
    names
}

/// The variable at the start of `a`, `a.b.c` or `a[i].b`.
fn root_variable(target: &SyntaxNode) -> Option<SyntaxNode> {
    match target.kind() {
        SyntaxKind::NameExpression => Some(target.clone()),
        SyntaxKind::FieldExpression | SyntaxKind::IndexExpression => root_variable(&expressions(target).next()?),
        _ => None,
    }
}

/// The expressions before the `=` of an assignment.
pub(super) fn assignment_targets(statement: &SyntaxNode) -> Vec<SyntaxNode> {
    expressions(statement).collect()
}
