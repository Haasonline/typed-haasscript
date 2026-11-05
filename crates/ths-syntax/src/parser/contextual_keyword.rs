//! The words the typed dialect treats as keywords only where they begin or
//! join a construct, such as `type` before a type alias. Anywhere else they
//! are ordinary names, so the lexer leaves them as identifiers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextualKeyword {
    As,
    Declare,
    Export,
    Extends,
    From,
    Import,
    Interface,
    Type,
}

impl ContextualKeyword {
    /// The word as written in a script.
    pub(crate) const fn text(self) -> &'static str {
        match self {
            ContextualKeyword::As => "as",
            ContextualKeyword::Declare => "declare",
            ContextualKeyword::Export => "export",
            ContextualKeyword::Extends => "extends",
            ContextualKeyword::From => "from",
            ContextualKeyword::Import => "import",
            ContextualKeyword::Interface => "interface",
            ContextualKeyword::Type => "type",
        }
    }
}
