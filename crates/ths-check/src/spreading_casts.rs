//! Where a cast on a call (or `...`) would hide values. The checker reads
//! `f() as T` as one value, but stripping leaves `f()`, which still passes
//! every value it returns wherever Lua keeps them. Parentheses truncate in
//! Lua and survive stripping, so `(f()) as T` is the fix.

use ths_syntax::{SyntaxKind, SyntaxNode};

use crate::syntax_access::{child, children, expressions, first_type, is_expression};

/// `f() as (number, string)`: a cast to several values, which keeps every
/// value of the call, as the stripped code does.
pub(crate) fn is_list_cast(cast: &SyntaxNode) -> bool {
    first_type(cast).is_some_and(|target| matches!(target.kind(), SyntaxKind::TypeList | SyntaxKind::VariadicType))
}

pub(crate) const SPREADING_CAST_MESSAGE: &str =
    "Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first.";

/// Whether the values after the first of `cast`'s operand reach something
/// at runtime: `cast` ends an argument list, a `return`, a table
/// constructor, or a local or assignment with more targets than
/// expressions.
pub(crate) fn extra_values_are_kept(cast: &SyntaxNode) -> bool {
    let Some(parent) = cast.parent() else {
        return false;
    };
    match parent.kind() {
        SyntaxKind::ExpressionList => {
            let values: Vec<SyntaxNode> = expressions(&parent).collect();
            if values.last() != Some(cast) {
                return false;
            }
            match parent.parent() {
                Some(statement) => match statement.kind() {
                    SyntaxKind::LocalStatement => {
                        let names = child(&statement, SyntaxKind::NameList)
                            .map_or(0, |list| children(&list, SyntaxKind::NameBinding).count());
                        names > values.len()
                    }
                    SyntaxKind::AssignmentStatement => {
                        let targets = statement.children().filter(|node| is_expression(node.kind())).count();
                        targets > values.len()
                    }
                    _ => true,
                },
                None => false,
            }
        }
        SyntaxKind::PositionalField => parent.parent().is_some_and(|table| {
            table.kind() == SyntaxKind::TableConstructor && table.children().last().as_ref() == Some(&parent)
        }),
        _ => false,
    }
}
