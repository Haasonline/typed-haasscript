//! Declaration files a checked script sees besides its own declarations,
//! such as the TradeServer's built-in commands and types.

use ths_syntax::{Dialect, SyntaxNode, parse};

/// Generated from the TradeServer's command list.
pub(crate) const GENERATED_DECLARATIONS: &str = include_str!("../declarations/haasscript.d.ths");
/// Hand-written corrections; the generated file leaves out their names.
pub(crate) const OVERRIDE_DECLARATIONS: &str = include_str!("../declarations/overrides.d.ths");
/// Lua's own libraries, hand-written.
pub(crate) const LUA_DECLARATIONS: &str = include_str!("../declarations/lua.d.ths");

/// Parsed declaration files. Diagnostics inside them are never reported:
/// the author of the checked script cannot fix them.
#[derive(Debug, Default, Clone)]
pub struct Declarations {
    roots: Vec<SyntaxNode>,
    /// Every name the TradeServer's compile check knows is declared, so a
    /// name none of them nor the script defines is an unknown reference.
    covers_server_names: bool,
}

impl Declarations {
    /// The TradeServer's built-in commands, constants and engine types,
    /// and Lua's libraries.
    pub fn built_in() -> Self {
        Declarations {
            covers_server_names: true,
            ..Self::from_sources(&[OVERRIDE_DECLARATIONS, GENERATED_DECLARATIONS, LUA_DECLARATIONS])
        }
    }

    /// Declaration files given as Typed HaasScript sources.
    pub fn from_sources(sources: &[&str]) -> Self {
        Declarations {
            roots: sources.iter().map(|source| parse(source, Dialect::Typed).syntax_node()).collect(),
            covers_server_names: false,
        }
    }

    /// These declarations and `sources` too, such as a project's own
    /// declaration files and the `CC_` declarations of its custom commands.
    pub fn with_sources(mut self, sources: &[&str]) -> Self {
        self.roots.extend(sources.iter().map(|source| parse(source, Dialect::Typed).syntax_node()));
        self
    }

    /// These declarations and `other`'s files too, already parsed.
    pub fn with_declarations(mut self, other: &Declarations) -> Self {
        self.roots.extend(other.roots.iter().cloned());
        self
    }

    pub(crate) fn roots(&self) -> &[SyntaxNode] {
        &self.roots
    }

    pub(crate) fn covers_server_names(&self) -> bool {
        self.covers_server_names
    }
}
