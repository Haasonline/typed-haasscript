//! The TradeServer refuses to compile a script that reads a name nothing
//! defines ("Unknown references: …"), checked statically, wherever the read
//! is (verified on the TradeServer). A name is known when a local of it is
//! in scope, the server declares it, the script assigns it as a global or
//! defines it with `function name()` anywhere (even after the read), or a
//! string literal is exactly the name. A comment, a table key, a field, a
//! longer string, a parameter or a `local` in another block does not make
//! it known.
//!
//! A `local` ends with its block, as in Lua 5.2: a read after the block
//! falls back to an outer or global definition of the name. That is often
//! intended (shadowing), so it is a warning only with the `shadowing`
//! setting.

use std::collections::HashSet;

use rowan::TextRange;
use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use super::control_flow::assignment_targets;
use crate::command_declaration::CUSTOM_COMMAND_PREFIX;
use crate::literals::literal_type;
use crate::syntax_access::{child, first_name, tokens};
use crate::types::Type;

/// The names `root` makes known to the server's compile check without a
/// local in scope: global assignments, global `function name()`
/// statements and the values of string literals. An assignment to a name
/// a local holds is counted too, which can only miss an unknown name.
pub(super) fn collect(root: &SyntaxNode) -> HashSet<String> {
    let mut known = HashSet::new();
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::AssignmentStatement => {
                let assigned = assignment_targets(&node)
                    .into_iter()
                    .filter(|target| target.kind() == SyntaxKind::NameExpression)
                    .filter_map(|target| first_name(&target));
                known.extend(assigned.map(|name| name.text().to_string()));
            }
            SyntaxKind::FunctionStatement => {
                let names: Vec<_> = child(&node, SyntaxKind::FunctionName)
                    .into_iter()
                    .flat_map(|name| tokens(&name).collect::<Vec<_>>())
                    .filter(|token| token.kind() == SyntaxKind::Identifier)
                    .collect();
                if let [name] = names.as_slice() {
                    known.insert(name.text().to_string());
                }
            }
            _ => {}
        }
    }
    let strings = root.descendants_with_tokens().filter_map(|element| element.into_token());
    known.extend(strings.filter_map(|token| match literal_type(&token) {
        Some(Type::StringLiteral(value)) => Some(value),
        _ => None,
    }));
    known
}

impl Checker<'_> {
    /// Applies each rule the check has turned on to a read of `name` at
    /// `range`.
    pub(super) fn check_name_read(&mut self, name: &str, range: TextRange) {
        if self.rules.unknown_names {
            self.check_unknown_name(name, range);
        }
        if self.rules.shadowing {
            self.check_ended_local_fallback(name, range);
        }
    }

    /// Whether the server's compile check knows `name` where it is read.
    fn is_known_name(&self, name: &str) -> bool {
        // A user's own custom commands need not be declared: the server
        // knows every command its owner has saved.
        self.scopes.lookup(name).is_some() || self.known_names.contains(name) || name.starts_with(CUSTOM_COMMAND_PREFIX)
    }

    /// Reports a read of `name` at `range` that the server would refuse to
    /// compile.
    fn check_unknown_name(&mut self, name: &str, range: TextRange) {
        if self.is_known_name(name) {
            return;
        }
        let ended_hint = if self.scopes.ended_local(name).is_some() {
            format!(" A local '{name}' above ended with its block.")
        } else {
            String::new()
        };
        self.sink.rejection(
            range,
            format!(
                "Cannot find name '{name}': the server refuses to compile a read of a name that nothing defines.{ended_hint}"
            ),
        );
    }

    /// Warns about a read of a known `name` at `range` that falls back past
    /// a `local` of the same name that has ended.
    fn check_ended_local_fallback(&mut self, name: &str, range: TextRange) {
        if !self.is_known_name(name) {
            return;
        }
        let resolved = self.scopes.lookup(name);
        // Symbols are numbered in declaration order, so a resolved symbol
        // older than the ended local is one the local was hiding.
        let falls_back =
            self.scopes.ended_local(name).is_some_and(|ended| resolved.is_none_or(|resolved| resolved < ended));
        if falls_back {
            self.sink.warning(
                range,
                format!("'{name}' here is the outer '{name}': the local '{name}' declared in a block above ended with its block."),
            );
        }
    }
}
