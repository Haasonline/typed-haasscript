//! Builds a lossless syntax tree from tokens. This file holds the token
//! cursor and tree-building machinery; the grammar lives in the submodules.

mod contextual_keyword;
mod expressions;
mod modules;
mod statements;
mod types;

use rowan::{Checkpoint, GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::lexer::{LexError, Token, lex};
use crate::parser::contextual_keyword::ContextualKeyword;
use crate::syntax_kind::SyntaxKind;
use crate::syntax_tree::SyntaxNode;

/// Which language a file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// HaasScript exactly as the TradeServer accepts it (`.lua`, `.hs`).
    Plain,
    /// HaasScript with type annotations (`.ths`, `.d.ths`).
    Typed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    pub errors: Vec<ParseError>,
}

impl Parse {
    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }
}

pub fn parse(source: &str, dialect: Dialect) -> Parse {
    let mut parser = Parser::new(source, dialect);
    parser.builder.start_node(SyntaxKind::Root.into());
    statements::chunk(&mut parser);
    parser.flush_trivia();
    parser.builder.finish_node();
    Parse { green: parser.builder.finish(), errors: parser.errors }
}

/// A lexer error reported as a parse error, so callers see one list.
fn parse_error_from_lex_error(error: LexError) -> ParseError {
    ParseError { message: error.kind.to_string(), range: error.range }
}

pub(crate) struct Parser<'source> {
    source: &'source str,
    tokens: Vec<Token>,
    /// Index into `tokens` of the next token not yet in the tree.
    position: usize,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
    dialect: Dialect,
}

impl<'source> Parser<'source> {
    /// A parser positioned at the first token of `source`, holding the
    /// lexer's errors so far.
    fn new(source: &'source str, dialect: Dialect) -> Self {
        let lexed = lex(source);
        Parser {
            source,
            tokens: lexed.tokens,
            position: 0,
            builder: GreenNodeBuilder::new(),
            errors: lexed.errors.into_iter().map(parse_error_from_lex_error).collect(),
            dialect,
        }
    }
}

impl Parser<'_> {
    /// Index into `tokens` of the `n`th upcoming token that is not trivia.
    fn nth_index(&self, n: usize) -> Option<usize> {
        self.tokens[self.position..]
            .iter()
            .enumerate()
            .filter(|(_, token)| !token.kind.is_trivia())
            .nth(n)
            .map(|(offset, _)| self.position + offset)
    }

    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        self.nth_index(n).map_or(SyntaxKind::EndOfFile, |index| self.tokens[index].kind)
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    /// True when the `n`th upcoming token is `keyword`, which the lexer
    /// leaves as a name since it is a keyword only in some positions.
    pub(crate) fn nth_is_keyword(&self, n: usize, keyword: ContextualKeyword) -> bool {
        self.nth_index(n).is_some_and(|index| {
            let token = self.tokens[index];
            token.kind == SyntaxKind::Identifier && &self.source[token.range] == keyword.text()
        })
    }

    fn current_range(&self) -> TextRange {
        match self.nth_index(0) {
            Some(index) => self.tokens[index].range,
            None => TextRange::empty(TextSize::of(self.source)),
        }
    }

    /// Adds pending trivia to the tree, so the next node starts at a real
    /// token.
    fn flush_trivia(&mut self) {
        while let Some(token) = self.tokens.get(self.position)
            && token.kind.is_trivia()
        {
            self.builder.token(token.kind.into(), &self.source[token.range]);
            self.position += 1;
        }
    }

    pub(crate) fn bump(&mut self) {
        self.flush_trivia();
        if let Some(token) = self.tokens.get(self.position) {
            self.builder.token(token.kind.into(), &self.source[token.range]);
            self.position += 1;
        }
    }

    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            return true;
        }
        false
    }

    /// Consumes `kind` or reports that `description` was expected.
    pub(crate) fn expect(&mut self, kind: SyntaxKind, description: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(format!("expected {description}"));
        false
    }

    pub(crate) fn error(&mut self, message: String) {
        let range = self.current_range();
        self.errors.push(ParseError { message, range });
    }

    /// Reports `message` and moves the current token into an error node, so
    /// parsing always makes progress.
    pub(crate) fn error_and_bump(&mut self, message: String) {
        self.error(message);
        if self.at(SyntaxKind::EndOfFile) {
            return;
        }
        self.start_node(SyntaxKind::ErrorNode);
        self.bump();
        self.finish_node();
    }

    pub(crate) fn is_typed(&self) -> bool {
        self.dialect == Dialect::Typed
    }

    /// Reports `feature` as an error in plain HaasScript. The syntax is still
    /// parsed, so the tree and later errors stay meaningful.
    pub(crate) fn require_typed_dialect(&mut self, feature: &str) {
        if self.dialect == Dialect::Plain {
            self.error(format!("{feature} are only allowed in .ths files"));
        }
    }

    /// Splits the current token after its first byte into `first` and
    /// `second`, as when `>=` closes type arguments: `Array<number>= {}`.
    pub(crate) fn split_current_token(&mut self, first: SyntaxKind, second: SyntaxKind) {
        let Some(index) = self.nth_index(0) else {
            return;
        };
        let range = self.tokens[index].range;
        let middle = range.start() + TextSize::from(1);
        self.tokens[index] = Token { kind: first, range: TextRange::new(range.start(), middle) };
        let rest = Token { kind: second, range: TextRange::new(middle, range.end()) };
        self.tokens.insert(index + 1, rest);
    }

    /// True when the current token is `(` and the token after its matching
    /// `)` is `kind`.
    pub(crate) fn parenthesis_is_followed_by(&self, kind: SyntaxKind) -> bool {
        let mut depth = 0usize;
        let mut n = 0;
        loop {
            match self.nth(n) {
                SyntaxKind::LeftParenthesis => depth += 1,
                SyntaxKind::RightParenthesis => {
                    depth -= 1;
                    if depth == 0 {
                        return self.nth(n + 1) == kind;
                    }
                }
                SyntaxKind::EndOfFile => return false,
                _ => {}
            }
            if depth == 0 {
                return false;
            }
            n += 1;
        }
    }

    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        self.flush_trivia();
        self.builder.start_node(kind.into());
    }

    pub(crate) fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    pub(crate) fn checkpoint(&mut self) -> Checkpoint {
        self.flush_trivia();
        self.builder.checkpoint()
    }

    pub(crate) fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, kind.into());
    }
}
