//! Checking statements: declarations, assignments and control flow.

use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::control_flow::{assigned_names, assignment_targets, block_always_exits};
use super::series::{SERIES_READ_ONLY_MESSAGE, is_series};
use super::value_lists::{ExpressionList, Supplied};
use super::{Checker, implicit_declared_type};
use crate::narrowing::ConditionNarrowings;
use crate::scopes::{NarrowingKey, SymbolId};
use crate::syntax_access::{child, children, expressions, first_expression, first_name, tokens};
use crate::types::Type;
use crate::visibility::BindingSite;

/// What an assignment target accepts.
struct AssignmentTarget {
    declared: Type,
    /// Something inferred also accepts nil: a variable (see
    /// `Scopes::declare_implicit`) or a field of an inferred table.
    accepts_nil: bool,
}

struct LocalDeclaration {
    declared: Type,
    /// The type right after the declaration, from its initializer.
    narrowed: Option<Type>,
    implicit: bool,
}

/// A local without an annotation, initialized with `initial`.
fn implicit_local(initial: Type) -> LocalDeclaration {
    LocalDeclaration { declared: implicit_declared_type(&initial), narrowed: Some(initial), implicit: true }
}

impl Checker<'_> {
    pub(super) fn check_block(&mut self, block: &SyntaxNode) {
        for statement in block.children() {
            self.check_statement(&statement);
        }
    }

    /// A block in a scope of its own.
    fn check_scoped_block(&mut self, block: Option<SyntaxNode>) {
        let Some(block) = block else {
            return;
        };
        self.scopes.push();
        self.check_block(&block);
        self.scopes.pop();
    }

    fn check_statement(&mut self, statement: &SyntaxNode) {
        match statement.kind() {
            SyntaxKind::LocalStatement => self.check_local(statement),
            SyntaxKind::LocalFunctionStatement => self.check_local_function(statement),
            SyntaxKind::FunctionStatement => self.check_function_statement(statement),
            SyntaxKind::AssignmentStatement => self.check_assignment(statement),
            SyntaxKind::CallStatement => {
                if let Some(call) = first_expression(statement) {
                    self.check_expression(&call, None);
                }
            }
            SyntaxKind::DoStatement => self.check_scoped_block(child(statement, SyntaxKind::Block)),
            SyntaxKind::IfStatement => self.check_if(statement),
            SyntaxKind::WhileStatement => self.check_while(statement),
            SyntaxKind::RepeatStatement => self.check_repeat(statement),
            SyntaxKind::NumericForStatement => self.check_numeric_for(statement),
            SyntaxKind::GenericForStatement => self.check_generic_for(statement),
            SyntaxKind::ReturnStatement => self.check_return(statement),
            // The declaration itself is checked as any other; what being
            // exported adds is checked with the module's imports.
            SyntaxKind::ExportDeclaration => {
                for declaration in statement.children() {
                    self.check_statement(&declaration);
                }
            }
            _ => {}
        }
    }

    fn check_local(&mut self, statement: &SyntaxNode) {
        let bindings: Vec<SyntaxNode> = child(statement, SyntaxKind::NameList)
            .map(|list| children(&list, SyntaxKind::NameBinding).collect())
            .unwrap_or_default();
        let mut values = ExpressionList::of(statement);
        // Every initializer is evaluated before any name is in scope:
        // `local x = x` reads the outer `x`.
        let mut declarations = Vec::new();
        for (index, binding) in bindings.iter().enumerate() {
            let annotated = self.annotation(binding, SyntaxKind::TypeAnnotation);
            let declaration = match (annotated, self.supplied(&mut values, index)) {
                (Some(declared), Supplied::Expression(value)) => {
                    let initial = self.check_assignable_expression(&value, &declared);
                    let narrowed = self.narrowed_by_assignment(&declared, &initial);
                    LocalDeclaration { declared, narrowed: Some(narrowed), implicit: false }
                }
                (Some(declared), Supplied::Spread(spread)) => {
                    let initial = spread.received();
                    self.report_if_not_assignable(&initial, &declared, spread.range);
                    let narrowed = self.narrowed_by_assignment(&declared, &initial);
                    LocalDeclaration { declared, narrowed: Some(narrowed), implicit: false }
                }
                (Some(declared), Supplied::Missing) => LocalDeclaration { declared, narrowed: None, implicit: false },
                (None, Supplied::Expression(value)) => {
                    let initial = self.check_expression(&value, None).widened();
                    implicit_local(initial)
                }
                (None, Supplied::Spread(spread)) => implicit_local(spread.received().widened()),
                (None, Supplied::Missing) => {
                    self.report_implicit_local(binding);
                    LocalDeclaration { declared: Type::Any, narrowed: None, implicit: true }
                }
            };
            declarations.push((binding.clone(), declaration));
        }
        self.check_enough_values(&values, bindings.len());
        self.check_untaken(&values, bindings.len());
        for (binding, declaration) in declarations {
            let Some(name) = first_name(&binding) else {
                continue;
            };
            let id = if declaration.implicit {
                self.scopes.declare_implicit(name.text(), declaration.declared)
            } else {
                self.scopes.declare(name.text(), declaration.declared)
            };
            self.note_declared_local(name.text(), id);
            self.record_local(id, &name, BindingSite::Local, statement);
            if let Some(narrowed) = declaration.narrowed {
                self.scopes.narrow(NarrowingKey::variable(id), narrowed);
            }
        }
    }

    fn check_local_function(&mut self, statement: &SyntaxNode) {
        let (Some(name), Some(body)) = (first_name(statement), child(statement, SyntaxKind::FunctionBody)) else {
            return;
        };
        // Declared before its body is checked, so the function can recurse.
        let signature = self.declared_signature(&body);
        let id = self.scopes.declare(name.text(), Type::Function(std::rc::Rc::new(signature)));
        self.note_declared_local(name.text(), id);
        self.record_local(id, &name, BindingSite::LocalFunction, statement);
        let checked = self.check_function_body(&body, None, None);
        self.scopes.redeclare(id, Type::Function(std::rc::Rc::new(checked)));
    }

    /// `function name()`, `function a.b.c()` or `function a.b:method()`.
    fn check_function_statement(&mut self, statement: &SyntaxNode) {
        let (Some(name), Some(body)) =
            (child(statement, SyntaxKind::FunctionName), child(statement, SyntaxKind::FunctionBody))
        else {
            return;
        };
        let names: Vec<_> = tokens(&name).filter(|token| token.kind() == SyntaxKind::Identifier).collect();
        let is_method = tokens(&name).any(|token| token.kind() == SyntaxKind::Colon);
        match names.as_slice() {
            // `function ()`, a name not typed yet: the parser reports it, and
            // there is nothing to assign, but the body is still checked.
            [] => {
                self.check_function_body(&body, None, None);
            }
            [name] => self.check_variable_function(statement, name, &body),
            path => self.check_field_function(path, is_method, &body),
        }
    }

    /// `function name()`: assigns the variable `name`, creating a global
    /// when nothing declares it.
    fn check_variable_function(&mut self, statement: &SyntaxNode, name: &SyntaxToken, body: &SyntaxNode) {
        self.check_command_global(name);
        self.check_read_only_assignment(name);
        let checked = self.check_function_body(body, None, None);
        let function = Type::Function(std::rc::Rc::new(checked));
        match self.scopes.lookup(name.text()) {
            Some(id) => {
                let declared = self.scopes.declared_type(id).clone();
                self.record_read(name, id, &declared);
                self.report_if_not_assignable(&function, &declared, name.text_range());
            }
            None => {
                let id = self.scopes.declare_implicit_global(name.text(), function);
                self.record_global(id, name, statement);
            }
        }
    }

    /// `function a.b.c()` or `function a.b:method()`: stores the function
    /// in a field of the table that `names`, the whole path, leads to.
    fn check_field_function(&mut self, names: &[SyntaxToken], is_method: bool, body: &SyntaxNode) {
        // The table the function is stored in, found by following the path.
        let mut owner = match self.scopes.lookup(names[0].text()) {
            Some(id) => {
                let current = self.scopes.current_type(id).clone();
                self.record_read(&names[0], id, &current);
                current
            }
            None => Type::Any,
        };
        for field in &names[1..names.len() - 1] {
            owner = self.member_type(&owner, field.text(), field.text_range());
            self.record_field(field, &owner);
        }
        let self_type = is_method.then(|| owner.clone());
        let last = names.last().expect("at least two names");
        let expected = match self.expand(&owner) {
            Type::Object(object) => {
                object.properties.get(last.text()).and_then(|property| match self.expand(&property.value) {
                    Type::Function(function) => Some((*function).clone()),
                    _ => None,
                })
            }
            _ => None,
        };
        let checked = self.check_function_body(body, expected.as_ref(), self_type);
        self.record_field(last, &Type::Function(std::rc::Rc::new(checked)));
    }

    fn check_assignment(&mut self, statement: &SyntaxNode) {
        let targets = assignment_targets(statement);
        let mut values = ExpressionList::of(statement);
        for (index, target) in targets.iter().enumerate() {
            if target.kind() == SyntaxKind::NameExpression
                && let Some(name) = first_name(target)
            {
                self.check_command_global(&name);
                self.check_read_only_assignment(&name);
            }
            let expected = self.check_assignment_target(target);
            let (assigned, range) = match self.supplied(&mut values, index) {
                Supplied::Expression(value) => {
                    let assigned = self.check_expression(&value, expected.as_ref().map(|target| &target.declared));
                    (assigned, value.text_range())
                }
                Supplied::Spread(spread) => (spread.received(), spread.range),
                Supplied::Missing => {
                    self.narrow_after_assignment(target, expected.as_ref(), &Type::Any);
                    continue;
                }
            };
            let retyped = self
                .widen_for_series(target, &assigned)
                .or_else(|| self.loosen_for_plain_reassignment(target, &assigned));
            let expected = match (expected, retyped) {
                (Some(expected), Some(declared)) => Some(AssignmentTarget { declared, ..expected }),
                (expected, _) => expected,
            };
            if let Some(target) = &expected {
                let checked = if target.accepts_nil { assigned.without_nil() } else { assigned.clone() };
                self.report_if_not_assignable(&checked, &target.declared, range);
            }
            self.narrow_after_assignment(target, expected.as_ref(), &assigned);
        }
        self.check_enough_values(&values, targets.len());
        self.check_untaken(&values, targets.len());
    }

    /// After `target = assigned`, the target holds the assigned value. A
    /// variable's field paths are no longer known. Assigning to an
    /// undeclared name creates an inferred global.
    fn narrow_after_assignment(&mut self, target: &SyntaxNode, expected: Option<&AssignmentTarget>, assigned: &Type) {
        if target.kind() == SyntaxKind::NameExpression
            && let Some(name) = first_name(target)
            && self.scopes.lookup(name.text()).is_none()
        {
            let widened = assigned.widened();
            let id = self.scopes.declare_implicit_global(name.text(), implicit_declared_type(&widened));
            if let Some(statement) = target.ancestors().find(|node| node.kind() == SyntaxKind::AssignmentStatement) {
                self.record_global(id, &name, &statement);
            }
            self.scopes.narrow(NarrowingKey::variable(id), widened);
            return;
        }
        let Some((key, _)) = self.reference(target) else {
            return;
        };
        self.scopes.forget(&key);
        if assigned.members().iter().any(Type::is_any) {
            // An inferred type came only from an earlier value; an
            // unchecked value says nothing about what the target holds now.
            if expected.is_none_or(|target| target.accepts_nil) {
                self.scopes.narrow(key, Type::Any);
            }
            return;
        }
        let narrowed = match expected {
            Some(target) if !target.accepts_nil => self.narrowed_by_assignment(&target.declared, assigned),
            _ => assigned.widened(),
        };
        self.scopes.narrow(key, narrowed);
    }

    /// Checks `target`, the left side of an assignment, and gives what a
    /// value assigned to it must be, or `None` when any value is fine (a
    /// new global, or an unchecked table).
    fn check_assignment_target(&mut self, target: &SyntaxNode) -> Option<AssignmentTarget> {
        match target.kind() {
            SyntaxKind::NameExpression => self.check_name_target(target),
            SyntaxKind::FieldExpression => self.check_field_target(target),
            SyntaxKind::IndexExpression => self.check_index_target(target),
            _ => {
                self.check_expression(target, None);
                None
            }
        }
    }

    /// `name = value`: records the read of an existing variable.
    fn check_name_target(&mut self, target: &SyntaxNode) -> Option<AssignmentTarget> {
        let name = first_name(target)?;
        let id = self.scopes.lookup(name.text())?;
        let declared = self.scopes.declared_type(id).clone();
        self.record_read(&name, id, &declared);
        Some(self.variable_target(id))
    }

    /// What a value assigned to the variable `id` must be.
    fn variable_target(&self, id: SymbolId) -> AssignmentTarget {
        AssignmentTarget { declared: self.scopes.declared_type(id).clone(), accepts_nil: self.scopes.is_implicit(id) }
    }

    /// `owner.field = value`: checks the owner and the field.
    fn check_field_target(&mut self, target: &SyntaxNode) -> Option<AssignmentTarget> {
        let owner = first_expression(target)?;
        let field = tokens(target).filter(|token| token.kind() == SyntaxKind::Identifier).last()?;
        let owner_type = self.check_expression(&owner, None);
        let owner_type = self.require_not_nil(&owner_type, owner.text_range());
        let value = self.member_type(&owner_type, field.text(), field.text_range());
        self.record_field(&field, &value);
        self.field_target(&owner_type, value)
    }

    /// What a value assigned to a field of type `value` on `owner` must
    /// be; `None` for an unchecked field.
    fn field_target(&self, owner: &Type, value: Type) -> Option<AssignmentTarget> {
        let owner_is_inferred = matches!(self.expand(owner), Type::Object(object) if object.open);
        (!value.is_any()).then_some(AssignmentTarget { declared: value, accepts_nil: owner_is_inferred })
    }

    /// `owner[index] = value`: checks the owner, the index and whether
    /// the owner can be written through brackets.
    fn check_index_target(&mut self, target: &SyntaxNode) -> Option<AssignmentTarget> {
        let mut parts = expressions(target);
        let (owner, index) = (parts.next()?, parts.next()?);
        let owner_type = self.check_expression(&owner, None);
        let owner_type = self.require_not_nil(&owner_type, owner.text_range());
        let index_type = self.check_expression(&index, None);
        self.check_index_write(target, (&owner, &owner_type), (&index, &index_type));
        let declared = self.index_target(&owner_type)?;
        Some(AssignmentTarget { declared, accepts_nil: false })
    }

    /// Reports writing to `owner[index]`, the assignment target `target`,
    /// when the owner is read-only or cannot be indexed, or the key may be
    /// a series.
    fn check_index_write(
        &mut self,
        target: &SyntaxNode,
        (owner, owner_type): (&SyntaxNode, &Type),
        (index, index_type): (&SyntaxNode, &Type),
    ) {
        match self.expand(owner_type) {
            Type::Array(_) => self.check_table_key(index_type, index.text_range()),
            series if is_series(&series) => {
                self.sink.error(target.text_range(), SERIES_READ_ONLY_MESSAGE.to_string());
            }
            Type::Object(_) => {
                if !self.is_container_type(owner_type) {
                    self.check_table_key(index_type, index.text_range());
                }
            }
            _ => self.check_indexable(owner_type, owner.text_range()),
        }
    }

    /// The type a value written through brackets into `owner` must have;
    /// `None` when any value is fine or the write is not allowed.
    fn index_target(&self, owner: &Type) -> Option<Type> {
        match self.expand(owner) {
            Type::Array(element) => Some(*element),
            series if is_series(&series) => None,
            Type::Object(object) => object.index.as_ref().map(|(_, value)| value.clone()),
            _ => None,
        }
    }

    fn apply(&mut self, narrowed: &[(NarrowingKey, Type)]) {
        for (key, value) in narrowed {
            self.scopes.narrow(key.clone(), value.clone());
        }
    }

    /// Forgets narrowings of every variable `node` may assign to, and of
    /// their field paths.
    fn forget_assigned_in(&mut self, node: &SyntaxNode) {
        for name in assigned_names(node) {
            if let Some(id) = self.scopes.lookup(&name) {
                self.scopes.forget(&NarrowingKey::variable(id));
            }
        }
    }

    fn check_condition(&mut self, condition: &SyntaxNode) -> ConditionNarrowings {
        self.check_expression(condition, None);
        self.condition_narrowings(condition)
    }

    /// Each branch sees its own condition true and every earlier one false.
    /// Afterwards, narrowings survive only if no branch could have changed
    /// them; and if every branch exits and there is no `else`, the code
    /// after the `if` runs only when every condition was false.
    fn check_if(&mut self, statement: &SyntaxNode) {
        let before = self.scopes.narrowings();
        let mut all_false: Vec<(NarrowingKey, Type)> = Vec::new();
        let mut every_branch_exits = true;
        let branches = std::iter::once(statement.clone()).chain(children(statement, SyntaxKind::ElseifClause));
        for branch in branches {
            self.scopes.restore_narrowings(before.clone());
            self.apply(&all_false);
            let narrowed = match first_expression(&branch) {
                Some(condition) => self.check_condition(&condition),
                None => ConditionNarrowings::default(),
            };
            self.apply(&narrowed.when_true);
            let block = child(&branch, SyntaxKind::Block);
            every_branch_exits &= block.as_ref().is_some_and(block_always_exits);
            self.check_scoped_block(block);
            all_false.extend(narrowed.when_false);
        }
        let else_clause = child(statement, SyntaxKind::ElseClause);
        if let Some(else_clause) = &else_clause {
            self.scopes.restore_narrowings(before.clone());
            self.apply(&all_false);
            self.check_scoped_block(child(else_clause, SyntaxKind::Block));
        }
        self.scopes.restore_narrowings(before);
        self.forget_assigned_in(statement);
        if else_clause.is_none() && every_branch_exits {
            self.apply(&all_false);
        }
    }

    fn check_while(&mut self, statement: &SyntaxNode) {
        self.forget_assigned_in(statement);
        let before = self.scopes.narrowings();
        let narrowed = match first_expression(statement) {
            Some(condition) => self.check_condition(&condition),
            None => ConditionNarrowings::default(),
        };
        self.apply(&narrowed.when_true);
        self.check_scoped_block(child(statement, SyntaxKind::Block));
        self.scopes.restore_narrowings(before);
        self.apply(&narrowed.when_false);
    }

    /// The `until` condition can see the body's locals.
    fn check_repeat(&mut self, statement: &SyntaxNode) {
        self.forget_assigned_in(statement);
        let before = self.scopes.narrowings();
        self.scopes.push();
        if let Some(block) = child(statement, SyntaxKind::Block) {
            self.check_block(&block);
        }
        if let Some(condition) = first_expression(statement) {
            self.check_expression(&condition, None);
        }
        self.scopes.pop();
        self.scopes.restore_narrowings(before);
    }

    fn check_numeric_for(&mut self, statement: &SyntaxNode) {
        for bound in expressions(statement) {
            self.check_assignable_expression(&bound, &Type::Number);
        }
        self.forget_assigned_in(statement);
        let before = self.scopes.narrowings();
        self.scopes.push();
        if let Some(binding) = child(statement, SyntaxKind::NameBinding) {
            let declared = self.annotation(&binding, SyntaxKind::TypeAnnotation).unwrap_or(Type::Number);
            if let Some(name) = first_name(&binding) {
                let id = self.scopes.declare(name.text(), declared);
                self.record_local(id, &name, BindingSite::LoopVariable, statement);
            }
        }
        if let Some(block) = child(statement, SyntaxKind::Block) {
            self.check_block(&block);
        }
        self.scopes.pop();
        self.scopes.restore_narrowings(before);
    }

    fn check_generic_for(&mut self, statement: &SyntaxNode) {
        if let Some(list) = child(statement, SyntaxKind::ExpressionList) {
            for value in expressions(&list) {
                self.check_expression(&value, None);
            }
        }
        self.forget_assigned_in(statement);
        let before = self.scopes.narrowings();
        self.scopes.push();
        let bindings = child(statement, SyntaxKind::NameList)
            .map(|list| children(&list, SyntaxKind::NameBinding).collect::<Vec<_>>())
            .unwrap_or_default();
        for binding in bindings {
            let Some(name) = first_name(&binding) else {
                continue;
            };
            let id = match self.annotation(&binding, SyntaxKind::TypeAnnotation) {
                Some(declared) => self.scopes.declare(name.text(), declared),
                None => self.scopes.declare_implicit(name.text(), Type::Any),
            };
            self.record_local(id, &name, BindingSite::LoopVariable, statement);
        }
        if let Some(block) = child(statement, SyntaxKind::Block) {
            self.check_block(&block);
        }
        self.scopes.pop();
        self.scopes.restore_narrowings(before);
    }

    /// Each returned value is checked against its position in the declared
    /// return type, or its rest type; a missing one is nil.
    fn check_return(&mut self, statement: &SyntaxNode) {
        let mut values = ExpressionList::of(statement);
        let declared = self.functions.last().and_then(|function| function.declared_return.clone());
        let expected_count = declared.as_ref().map_or(0, Type::fixed_value_count);
        let mut returned = Vec::new();
        let mut index = 0;
        while index < expected_count || !values.is_exhausted(index) {
            let expected = declared.as_ref().and_then(|declared| declared.declared_value_at(index));
            let value = match (self.supplied(&mut values, index), &expected) {
                (Supplied::Expression(value), Some(expected)) => self.check_assignable_expression(&value, expected),
                (Supplied::Expression(value), None) => self.check_expression(&value, None),
                // A rest value passed on is checked against the rest type
                // when there is one: it is absent rather than nil.
                (Supplied::Spread(spread), Some(expected)) => {
                    let value = if index >= expected_count { spread.value.clone() } else { spread.received() };
                    self.report_if_not_assignable(&value, expected, spread.range);
                    value
                }
                (Supplied::Spread(spread), None) => spread.received(),
                (Supplied::Missing, Some(expected)) => {
                    self.report_if_not_assignable(&Type::Nil, expected, statement.text_range());
                    Type::Nil
                }
                (Supplied::Missing, None) => Type::Nil,
            };
            returned.push(value);
            index += 1;
        }
        let has_rest = declared.as_ref().is_some_and(Type::has_rest_values);
        if declared.is_some() && !has_rest && values.expression_count() > expected_count {
            self.sink.error(
                statement.text_range(),
                format!("Expected {expected_count} return value(s), but got {}.", values.expression_count()),
            );
        }
        if let Some(function) = self.functions.last_mut() {
            function.returned.push(returned);
        }
    }
}
