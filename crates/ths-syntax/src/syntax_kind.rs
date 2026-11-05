/// Every kind of token and node in a Typed HaasScript syntax tree.
///
/// Tokens come first, then nodes. `type`, `interface`, `as`, `import`,
/// `export` and `from` are not keywords: they are valid Lua names
/// (`type(x)` is a standard function), so they lex as `Identifier` and the
/// parser recognizes them by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Trivia
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    /// A first line starting with `#!`, which HaasScript skips.
    Shebang,
    /// A leading U+FEFF.
    ByteOrderMark,

    // Literals and names
    Number,
    String,
    LongString,
    Identifier,

    // Keywords
    AndKeyword,
    BreakKeyword,
    DoKeyword,
    ElseKeyword,
    ElseifKeyword,
    EndKeyword,
    FalseKeyword,
    ForKeyword,
    FunctionKeyword,
    GotoKeyword,
    IfKeyword,
    InKeyword,
    LocalKeyword,
    NilKeyword,
    NotKeyword,
    OrKeyword,
    RepeatKeyword,
    ReturnKeyword,
    ThenKeyword,
    TrueKeyword,
    UntilKeyword,
    WhileKeyword,

    // Operators and punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Hash,
    Ampersand,
    Pipe,
    Question,
    EqualsEquals,
    TildeEquals,
    /// `!=`, MoonSharp's alias for `~=`.
    BangEquals,
    LessEquals,
    GreaterEquals,
    Less,
    Greater,
    Equals,
    LeftParenthesis,
    RightParenthesis,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    DoubleColon,
    Semicolon,
    Colon,
    Comma,
    Dot,
    DoubleDot,
    TripleDot,
    /// `=>`, in function types.
    FatArrow,

    /// A character or sequence the lexer could not make into any token.
    Error,
    /// Never in a tree: what the parser sees past the last token.
    EndOfFile,

    // Nodes
    Root,
    Block,
    /// Tokens the parser skipped while recovering from an error.
    ErrorNode,

    // Statements
    EmptyStatement,
    LocalStatement,
    LocalFunctionStatement,
    FunctionStatement,
    AssignmentStatement,
    CallStatement,
    DoStatement,
    WhileStatement,
    RepeatStatement,
    IfStatement,
    ElseifClause,
    ElseClause,
    NumericForStatement,
    GenericForStatement,
    ReturnStatement,
    BreakStatement,
    GotoStatement,
    LabelStatement,

    // Statement parts
    NameList,
    /// One declared name in a `local` or `for` statement.
    NameBinding,
    ExpressionList,
    /// `a.b.c` or `a.b:c` after `function`.
    FunctionName,
    FunctionBody,
    ParameterList,
    Parameter,

    // Expressions
    NameExpression,
    LiteralExpression,
    VarargExpression,
    FunctionExpression,
    /// MoonSharp's short lambda: `|x, y| x + y`.
    LambdaExpression,
    TableConstructor,
    PositionalField,
    NamedField,
    IndexedField,
    BinaryExpression,
    UnaryExpression,
    ParenthesizedExpression,
    FieldExpression,
    IndexExpression,
    CallExpression,
    MethodCallExpression,
    ArgumentList,
    /// `expression as Type`.
    CastExpression,

    // Type declarations (typed dialect only)
    TypeAliasStatement,
    InterfaceStatement,
    /// `extends A, B` on an interface.
    ExtendsClause,
    DeclareFunctionStatement,
    DeclareVariableStatement,

    // Annotations (typed dialect only)
    /// `: Type` after a name or parameter.
    TypeAnnotation,
    /// `: Type` after a function's parameter list.
    ReturnTypeAnnotation,
    TypeParameterList,
    TypeParameter,
    TypeArgumentList,

    // Types
    NameType,
    LiteralType,
    ArrayType,
    UnionType,
    IntersectionType,
    ParenthesizedType,
    FunctionType,
    ObjectType,
    PropertySignature,
    MethodSignature,
    IndexSignature,
    /// `(A, B)`: several values, valid only as a return type.
    TypeList,
    /// `f<<number>>`: a generic function with explicit type arguments.
    InstantiationExpression,
    /// The `<<number, string>>` of an instantiation or method call.
    TypeInstantiation,
    /// `...string`: any number of values, valid only at the end of a
    /// return type.
    VariadicType,
    /// `declare type Name extends Base`: an opaque type, for declaration
    /// files.
    DeclareTypeStatement,
    /// `<T>(a: T): R` in an object type body: what calling the value does.
    CallSignature,

    // Modules (typed dialect only)
    /// `import { a, b as c } from "./path"` or `import * as m from "./path"`.
    ImportStatement,
    /// One name in an import's braces: `a`, `b as c`, `type T`.
    ImportSpecifier,
    /// `* as m`: every export as one table.
    NamespaceImport,
    /// `export` and the declaration it exports.
    ExportDeclaration,
    /// `export { a, b as c }`: existing locals, exported.
    ExportListStatement,
    /// One name in an export list: `a` or `b as c`.
    ExportSpecifier,
}

impl SyntaxKind {
    /// The last variant. Keep it in step when adding variants at the end.
    pub const LAST: SyntaxKind = SyntaxKind::ExportSpecifier;

    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::BlockComment
                | SyntaxKind::Shebang
                | SyntaxKind::ByteOrderMark
        )
    }

    pub fn keyword_from_name(name: &str) -> Option<SyntaxKind> {
        let kind = match name {
            "and" => SyntaxKind::AndKeyword,
            "break" => SyntaxKind::BreakKeyword,
            "do" => SyntaxKind::DoKeyword,
            "else" => SyntaxKind::ElseKeyword,
            "elseif" => SyntaxKind::ElseifKeyword,
            "end" => SyntaxKind::EndKeyword,
            "false" => SyntaxKind::FalseKeyword,
            "for" => SyntaxKind::ForKeyword,
            "function" => SyntaxKind::FunctionKeyword,
            "goto" => SyntaxKind::GotoKeyword,
            "if" => SyntaxKind::IfKeyword,
            "in" => SyntaxKind::InKeyword,
            "local" => SyntaxKind::LocalKeyword,
            "nil" => SyntaxKind::NilKeyword,
            "not" => SyntaxKind::NotKeyword,
            "or" => SyntaxKind::OrKeyword,
            "repeat" => SyntaxKind::RepeatKeyword,
            "return" => SyntaxKind::ReturnKeyword,
            "then" => SyntaxKind::ThenKeyword,
            "true" => SyntaxKind::TrueKeyword,
            "until" => SyntaxKind::UntilKeyword,
            "while" => SyntaxKind::WhileKeyword,
            _ => return None,
        };
        Some(kind)
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        rowan::SyntaxKind(kind as u16)
    }
}
