//! In a plain file, an inferred variable or field later assigned a value of
//! another kind becomes `any` instead of being an error. A plain file
//! cannot hold the annotation that would state the intent, and it is
//! checked only so that typed files can use what it defines. A `.ths` file
//! keeps the inferred type and reports it.

use ths_syntax::SyntaxNode;

use super::Checker;
use crate::types::Type;

impl Checker<'_> {
    /// Makes the inferred `target` `any` when it cannot hold `assigned`,
    /// in a plain file only, and returns its new declared type.
    pub(super) fn loosen_for_plain_reassignment(&mut self, target: &SyntaxNode, assigned: &Type) -> Option<Type> {
        if !self.rules.lenient {
            return None;
        }
        self.retype_inferred(target, assigned, |checker, declared, assigned| {
            (!checker.is_assignable(&assigned.without_nil(), declared)).then_some(Type::Any)
        })
    }
}
