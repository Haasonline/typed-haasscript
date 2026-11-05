//! Lossless lexer and parser for Typed HaasScript: HaasScript (MoonSharp's
//! Lua 5.2 dialect) plus TypeScript-style type annotations.

pub mod lexer;
pub mod line_column;
pub mod parser;
pub mod syntax_kind;
pub mod syntax_tree;

pub use line_column::{LineColumn, line_column};
pub use parser::{Dialect, Parse, ParseError, parse};
pub use syntax_kind::SyntaxKind;
pub use syntax_tree::{SyntaxElement, SyntaxNode, SyntaxToken};

#[cfg(test)]
mod lexer_tests;
#[cfg(test)]
mod parser_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod typed_parser_tests;
