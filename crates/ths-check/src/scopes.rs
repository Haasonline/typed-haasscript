//! Variables in scope, their declared types, and what control flow has
//! narrowed them (or fields reached from them) to at the current point.

use std::collections::HashMap;

use crate::types::Type;

pub(crate) type SymbolId = usize;

/// What a narrowing applies to: a variable, or a field path from one
/// (`order.price` is the variable `order` with the path `["price"]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NarrowingKey {
    pub(crate) symbol: SymbolId,
    pub(crate) path: Vec<String>,
}

impl NarrowingKey {
    pub(crate) fn variable(symbol: SymbolId) -> Self {
        NarrowingKey { symbol, path: Vec::new() }
    }

    pub(crate) fn field(&self, name: &str) -> Self {
        let mut path = self.path.clone();
        path.push(name.to_string());
        NarrowingKey { symbol: self.symbol, path }
    }
}

#[derive(Debug)]
struct Symbol {
    declared: Type,
    /// No annotation gave the type; it was inferred. Such a variable also
    /// accepts nil, as Lua code clears variables with `x = nil`.
    implicit: bool,
    /// Created by assigning to a name nothing declared.
    assigned_global: bool,
    read_only: Option<ReadOnly>,
}

/// Why a variable may not be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadOnly {
    /// An import: a copy of another module's export.
    Import,
    /// A module's export: importers hold a copy taken when the module
    /// finished loading.
    Export,
}

/// Narrowed types. A key with no entry has its declared type.
pub(crate) type Narrowings = HashMap<NarrowingKey, Type>;

#[derive(Debug)]
pub(crate) struct Scopes {
    symbols: Vec<Symbol>,
    /// Innermost scope last; the first is the file's global scope.
    stack: Vec<HashMap<String, SymbolId>>,
    /// Globals from `declare` statements, which top-level locals sharing
    /// the global scope do not replace.
    declared_globals: HashMap<String, SymbolId>,
    /// For each scope on the stack, the locals of blocks inside it that
    /// have ended, the latest of each name.
    ended: Vec<HashMap<String, SymbolId>>,
    narrowed: Narrowings,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Scopes {
            symbols: Vec::new(),
            stack: vec![HashMap::new()],
            declared_globals: HashMap::new(),
            ended: vec![HashMap::new()],
            narrowed: HashMap::new(),
        }
    }

    pub(crate) fn push(&mut self) {
        self.stack.push(HashMap::new());
        self.ended.push(HashMap::new());
    }

    /// Ends the innermost scope; its locals, and those of blocks that ended
    /// inside it, are remembered by the enclosing scope as ended.
    pub(crate) fn pop(&mut self) {
        let (Some(locals), Some(ended_inside)) = (self.stack.pop(), self.ended.pop()) else {
            return;
        };
        let enclosing = self.ended.last_mut().expect("the global scope is never popped");
        for (name, id) in locals.into_iter().chain(ended_inside) {
            let latest = enclosing.entry(name).or_insert(id);
            *latest = (*latest).max(id);
        }
    }

    /// The latest local named `name` whose block has ended, inside a scope
    /// still on the stack.
    pub(crate) fn ended_local(&self, name: &str) -> Option<SymbolId> {
        self.ended.iter().filter_map(|ended| ended.get(name).copied()).max()
    }

    /// A variable whose type an annotation (or a declaration) gave.
    pub(crate) fn declare(&mut self, name: &str, declared: Type) -> SymbolId {
        self.declare_in_innermost(name, declared, false)
    }

    /// A variable whose type was inferred.
    pub(crate) fn declare_implicit(&mut self, name: &str, declared: Type) -> SymbolId {
        self.declare_in_innermost(name, declared, true)
    }

    fn declare_in_innermost(&mut self, name: &str, declared: Type, implicit: bool) -> SymbolId {
        let id = self.new_symbol(declared, implicit);
        self.stack.last_mut().expect("the global scope is never popped").insert(name.to_string(), id);
        id
    }

    /// A global from a `declare` statement.
    pub(crate) fn declare_global(&mut self, name: &str, declared: Type) -> SymbolId {
        let id = self.new_symbol(declared, false);
        self.stack[0].insert(name.to_string(), id);
        self.declared_globals.insert(name.to_string(), id);
        id
    }

    /// A global created by assigning to a name nothing declared.
    pub(crate) fn declare_implicit_global(&mut self, name: &str, declared: Type) -> SymbolId {
        let id = self.new_symbol(declared, true);
        self.symbols[id].assigned_global = true;
        self.stack[0].insert(name.to_string(), id);
        id
    }

    fn new_symbol(&mut self, declared: Type, implicit: bool) -> SymbolId {
        self.symbols.push(Symbol { declared, implicit, assigned_global: false, read_only: None });
        self.symbols.len() - 1
    }

    pub(crate) fn is_implicit(&self, id: SymbolId) -> bool {
        self.symbols[id].implicit
    }

    /// Whether the symbol is a global the script created by assigning to
    /// it, not a local or a declared global.
    pub(crate) fn is_assigned_global(&self, id: SymbolId) -> bool {
        self.symbols[id].assigned_global
    }

    pub(crate) fn mark_read_only(&mut self, id: SymbolId, reason: ReadOnly) {
        self.symbols[id].read_only = Some(reason);
    }

    pub(crate) fn read_only(&self, id: SymbolId) -> Option<ReadOnly> {
        self.symbols[id].read_only
    }

    /// Whether declarations now go into the file's own scope, outside every
    /// block and function.
    pub(crate) fn is_top_level(&self) -> bool {
        self.stack.len() == 1
    }

    /// A global from a `declare` statement, even where a local of the same
    /// name hides it.
    pub(crate) fn lookup_declared_global(&self, name: &str) -> Option<SymbolId> {
        self.declared_globals.get(name).copied()
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.stack.iter().rev().find_map(|scope| scope.get(name).copied())
    }

    /// Replaces a declared type, as when a function's return type has been
    /// inferred from its body.
    pub(crate) fn redeclare(&mut self, id: SymbolId, declared: Type) {
        self.symbols[id].declared = declared;
    }

    pub(crate) fn declared_type(&self, id: SymbolId) -> &Type {
        &self.symbols[id].declared
    }

    /// The variable's type at this point: narrowed if control flow narrowed it.
    pub(crate) fn current_type(&self, id: SymbolId) -> &Type {
        self.narrowed.get(&NarrowingKey::variable(id)).unwrap_or(&self.symbols[id].declared)
    }

    pub(crate) fn narrowed(&self, key: &NarrowingKey) -> Option<&Type> {
        self.narrowed.get(key)
    }

    pub(crate) fn narrow(&mut self, key: NarrowingKey, narrowed: Type) {
        self.narrowed.insert(key, narrowed);
    }

    /// Drops the narrowing of `key` and of every field path below it.
    pub(crate) fn forget(&mut self, key: &NarrowingKey) {
        self.narrowed.retain(|narrowed, _| narrowed.symbol != key.symbol || !narrowed.path.starts_with(&key.path));
    }

    pub(crate) fn narrowings(&self) -> Narrowings {
        self.narrowed.clone()
    }

    pub(crate) fn restore_narrowings(&mut self, narrowings: Narrowings) {
        self.narrowed = narrowings;
    }
}
