//! Binds `SyntaxKind` to rowan, giving the typed tree handles used
//! everywhere else.

use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HaasScriptLanguage {}

impl rowan::Language for HaasScriptLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        assert!(raw.0 <= SyntaxKind::LAST as u16, "unknown syntax kind {}", raw.0);
        // SAFETY: `SyntaxKind` is `repr(u16)` with contiguous discriminants
        // from 0 to `LAST`, and the bound is checked above.
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<HaasScriptLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<HaasScriptLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<HaasScriptLanguage>;
