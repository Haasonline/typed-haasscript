//! What an editor asks of a checked file, recorded while it was checked:
//! where each name is defined, the type of every name and field where it
//! is written, what each name and field holds, and which names are in
//! scope where. Ranges are byte ranges in the file's text.

use rowan::{TextRange, TextSize};

use crate::types::Type;

/// How a name was declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// `local x`, a `for` variable or a `local function`.
    Local,
    /// A function's parameter or `self`.
    Parameter,
    /// A global the script creates by assigning to it, or with `function f()`.
    Global,
    /// A global from a `declare` statement: a built-in or a project
    /// declaration file.
    Declared,
    /// A value imported from a module.
    Import,
}

/// Where an import's value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOrigin {
    /// The module path as the import writes it (`"./math/averages"`).
    pub module_path: String,
    /// The export's name; `None` for `* as name`, which is the whole module.
    pub exported: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    /// The name where it is declared, when that is in this file.
    pub definition: Option<TextRange>,
    /// Where the name can be read; `None` for the whole file.
    pub visible: Option<TextRange>,
    pub import: Option<ImportOrigin>,
    /// The comment lines right above its declaration.
    pub documentation: Option<String>,
    pub(crate) declared: Type,
}

impl AnalyzedSymbol {
    /// Its declared type, as messages write types.
    pub fn type_text(&self) -> String {
        self.declared.to_string()
    }

    pub fn is_function(&self) -> bool {
        matches!(self.declared, Type::Function(_))
    }
}

/// A name or field written in the file.
#[derive(Debug, Clone)]
pub struct Reference {
    /// The name itself.
    pub range: TextRange,
    pub target: ReferenceTarget,
    pub(crate) value: Type,
}

impl Reference {
    /// Its type where it is written, narrowing included.
    pub fn type_text(&self) -> String {
        self.value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTarget {
    /// An index into `FileAnalysis::symbols`.
    Symbol(usize),
    /// A field or method of a table: `name` in `owner.name` or `owner:name()`.
    Field(String),
}

/// Which of `.` and `:` a member site is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAccess {
    /// `owner.name`: any field.
    Field,
    /// `owner:name()`: a method, called with `owner` as `self`.
    Method,
}

/// A `.` or `:` after a value, where its fields and methods can be completed.
#[derive(Debug, Clone)]
pub struct MemberSite {
    /// Right after the `.` or `:`.
    pub after: TextSize,
    pub access: MemberAccess,
    /// The name written after it, if any yet.
    pub name: Option<TextRange>,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct Member {
    pub name: String,
    pub optional: bool,
    pub(crate) value: Type,
}

impl Member {
    pub fn type_text(&self) -> String {
        self.value.to_string()
    }

    pub fn is_function(&self) -> bool {
        matches!(self.value, Type::Function(_))
    }
}

impl MemberSite {
    /// The members that can be written here: after `:`, only those that
    /// can be called, so plainly data such as a number is left out.
    pub fn offered(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|member| self.access == MemberAccess::Field || could_be_called(&member.value))
    }
}

/// False only for a type that is plainly not a function; a named type or
/// a type parameter may stand for one.
fn could_be_called(value: &Type) -> bool {
    match value {
        Type::Union(members) => members.iter().any(could_be_called),
        Type::Nil
        | Type::Never
        | Type::Boolean
        | Type::Number
        | Type::String
        | Type::BooleanLiteral(_)
        | Type::NumberLiteral(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Object(_) => false,
        _ => true,
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileAnalysis {
    pub symbols: Vec<AnalyzedSymbol>,
    /// In source order.
    pub references: Vec<Reference>,
    /// In source order.
    pub member_sites: Vec<MemberSite>,
    /// The cursor offsets inside each comment and string, half-open.
    pub comment_and_string_interiors: Vec<TextRange>,
}

impl FileAnalysis {
    /// The name or field at `offset`, its end included, so a cursor right
    /// after a name finds it.
    pub fn reference_at(&self, offset: TextSize) -> Option<&Reference> {
        self.references.iter().find(|reference| reference.range.contains_inclusive(offset))
    }

    /// The symbol a reference names, if it names one.
    pub fn symbol_of(&self, reference: &Reference) -> Option<&AnalyzedSymbol> {
        match reference.target {
            ReferenceTarget::Symbol(index) => self.symbols.get(index),
            ReferenceTarget::Field(_) => None,
        }
    }

    /// The `.` or `:` whose member is being written at `offset`: right
    /// after it, or within the name that follows it.
    pub fn member_site_at(&self, offset: TextSize) -> Option<&MemberSite> {
        self.member_sites.iter().rev().find(|site| match site.name {
            Some(name) => site.after <= offset && offset <= name.end(),
            None => site.after == offset,
        })
    }

    /// Whether a cursor at `offset` is inside a comment or string, where
    /// no name is written.
    pub fn is_in_comment_or_string(&self, offset: TextSize) -> bool {
        self.comment_and_string_interiors.iter().any(|interior| interior.contains(offset))
    }

    /// The names that can be read at `offset`, one symbol for each name:
    /// the innermost, latest declaration, which hides the others.
    pub fn names_in_scope(&self, offset: TextSize) -> Vec<&AnalyzedSymbol> {
        let mut chosen: std::collections::HashMap<&str, &AnalyzedSymbol> = std::collections::HashMap::new();
        for symbol in &self.symbols {
            let is_visible = symbol.visible.is_none_or(|visible| visible.contains_inclusive(offset));
            if !is_visible {
                continue;
            }
            let replaces = match chosen.get(symbol.name.as_str()) {
                None => true,
                Some(current) => hides(symbol, current),
            };
            if replaces {
                chosen.insert(&symbol.name, symbol);
            }
        }
        let mut names: Vec<&AnalyzedSymbol> = chosen.into_values().collect();
        names.sort_by(|left, right| left.name.cmp(&right.name));
        names
    }
}

/// Whether `candidate` hides `current`, both visible at one point: a
/// narrower scope hides a wider one, and of two in the same scope the later
/// declaration does.
fn hides(candidate: &AnalyzedSymbol, current: &AnalyzedSymbol) -> bool {
    match (candidate.visible, current.visible) {
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (Some(candidate_scope), Some(current_scope)) => {
            candidate_scope.start() >= current_scope.start() && candidate_scope.end() <= current_scope.end()
        }
        (None, None) => candidate.definition.is_some() && current.definition.is_none(),
    }
}
