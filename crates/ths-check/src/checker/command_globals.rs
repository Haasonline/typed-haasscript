//! A custom command may not create globals. The server reads the command's
//! code, not what its compile pass runs (verified on the TradeServer): an
//! assignment to a name that is neither a local nor a built-in, or a
//! `function name()` statement for one, anywhere in the file, even in a
//! function never called or a branch never taken, fails the compile with
//! "Only local variables are allowed. Make sure they are defined `local`.
//! (name)", once per name. A field of a global table (`string.extra = 1`)
//! and a built-in name (`Log = Log`) compile. A file is a command when it
//! calls `DefineCommand`.

use std::collections::HashSet;

use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::Checker;
use crate::command_signature::{DEFINE_COMMAND, callee_name};

impl Checker<'_> {
    /// Starts watching for created globals when `root` defines a command.
    pub(super) fn note_command_file(&mut self, root: &SyntaxNode) {
        let defines_command = root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::CallExpression)
            .any(|call| callee_name(&call).as_deref() == Some(DEFINE_COMMAND));
        self.command_globals = defines_command.then(HashSet::new);
    }

    /// Reports `name`, about to be assigned, when it would create a global
    /// in a command. The server rejects it, so even a plain file reports it.
    pub(super) fn check_command_global(&mut self, name: &SyntaxToken) {
        let creates_global = match self.scopes.lookup(name.text()) {
            None => true,
            Some(id) => self.scopes.is_assigned_global(id),
        };
        let Some(reported) = &mut self.command_globals else { return };
        if !creates_global || !reported.insert(name.text().to_string()) {
            return;
        }
        self.sink.rejection(
            name.text_range(),
            format!(
                "Cannot assign to '{}': a custom command may not create globals, and the server refuses to compile one that does. Declare it 'local'.",
                name.text()
            ),
        );
    }
}
