//! Inferring and checking expression types, and operator rules.

use std::rc::Rc;

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use super::series::{is_series, is_series_operand, series_may_equal, series_type};
use crate::literals::literal_type;
use crate::spreading_casts::{SPREADING_CAST_MESSAGE, extra_values_are_kept, is_list_cast};
use crate::syntax_access::{child, expressions, first_expression, first_name, first_type, tokens};
use crate::types::{FunctionType, Type};

fn is_number(value: &Type) -> bool {
    matches!(value, Type::Number | Type::NumberLiteral(_))
}

impl Checker<'_> {
    /// The type of `expression`. `expected`, when known, guides table
    /// constructors and function expressions (contextual typing); the
    /// caller still checks the result against it.
    pub(super) fn check_expression(&mut self, expression: &SyntaxNode, expected: Option<&Type>) -> Type {
        let value = self.expression_type(expression, expected);
        self.note_type(expression, &value);
        value
    }

    /// Keeps the type of `expression` for whichever recorder wants it.
    pub(super) fn note_type(&mut self, expression: &SyntaxNode, value: &Type) {
        self.recorded.note(expression.text_range(), value);
        self.parameter_defaults.note(expression.text_range(), value);
    }

    fn expression_type(&mut self, expression: &SyntaxNode, expected: Option<&Type>) -> Type {
        match expression.kind() {
            SyntaxKind::LiteralExpression => {
                tokens(expression).next().and_then(|token| literal_type(&token)).unwrap_or(Type::Any)
            }
            SyntaxKind::VarargExpression => self.vararg_type(),
            SyntaxKind::NameExpression => self.name_type(expression),
            SyntaxKind::ParenthesizedExpression => match first_expression(expression) {
                Some(inner) => self.check_expression(&inner, expected),
                None => Type::Any,
            },
            SyntaxKind::FieldExpression => self.check_field(expression),
            SyntaxKind::IndexExpression => self.check_index(expression),
            // Inside an expression a call gives one value (see `value_lists`).
            SyntaxKind::CallExpression => self.check_call(expression).value_at(0),
            SyntaxKind::MethodCallExpression => self.check_method_call(expression).value_at(0),
            SyntaxKind::FunctionExpression => {
                let contextual = expected.and_then(|expected| self.expected_function(expected));
                match child(expression, SyntaxKind::FunctionBody) {
                    Some(body) => Type::Function(Rc::new(self.check_function_body(&body, contextual.as_ref(), None))),
                    None => Type::Any,
                }
            }
            SyntaxKind::LambdaExpression => {
                let contextual = expected.and_then(|expected| self.expected_function(expected));
                Type::Function(Rc::new(self.check_lambda(expression, contextual.as_ref())))
            }
            SyntaxKind::TableConstructor => self.check_table(expression, expected),
            SyntaxKind::BinaryExpression => self.check_binary(expression),
            SyntaxKind::UnaryExpression => self.check_unary(expression),
            SyntaxKind::CastExpression => self.check_cast(expression).value_at(0),
            SyntaxKind::InstantiationExpression => self.check_instantiation(expression),
            _ => Type::Any,
        }
    }

    /// The function type an expected type describes, for contextual typing.
    fn expected_function(&self, expected: &Type) -> Option<FunctionType> {
        self.expand(&expected.without_nil()).members().iter().find_map(|member| match self.expand(member) {
            Type::Function(function) => Some((*function).clone()),
            _ => None,
        })
    }

    /// An undeclared name is `any` until built-in declarations exist.
    fn name_type(&mut self, expression: &SyntaxNode) -> Type {
        let Some(name) = first_name(expression) else {
            return Type::Any;
        };
        self.check_name_read(name.text(), name.text_range());
        let Some(id) = self.scopes.lookup(name.text()) else {
            return Type::Any;
        };
        self.note_read(id);
        let current = self.scopes.current_type(id).clone();
        self.record_read(&name, id, &current);
        current
    }

    /// `owner.field`. The owner is checked even while the field is not
    /// written yet, so an editor can offer the owner's fields.
    fn check_field(&mut self, expression: &SyntaxNode) -> Type {
        let Some(owner) = first_expression(expression) else {
            return Type::Any;
        };
        let field = tokens(expression).filter(|token| token.kind() == SyntaxKind::Identifier).last();
        let owner_type = self.check_expression(&owner, None);
        if let Some(dot) = tokens(expression).find(|token| token.kind() == SyntaxKind::Dot) {
            self.record_member_site(&dot, field.as_ref(), &owner_type);
        }
        let Some(field) = field else {
            return Type::Any;
        };
        let owner_type = self.require_not_nil(&owner_type, owner.text_range());
        let member = self.member_type(&owner_type, field.text(), field.text_range());
        self.record_field(&field, &member);
        // A narrowed field path (`if order.price then ... order.price`).
        match self.reference(expression) {
            Some((key, _)) => self.scopes.narrowed(&key).cloned().unwrap_or(member),
            None => member,
        }
    }

    fn check_index(&mut self, expression: &SyntaxNode) -> Type {
        let mut parts = expressions(expression);
        let (Some(owner), Some(index)) = (parts.next(), parts.next()) else {
            return Type::Any;
        };
        let owner_type = self.check_expression(&owner, None);
        let owner_type = self.require_not_nil(&owner_type, owner.text_range());
        let index_type = self.check_expression(&index, None);
        match self.expand(&owner_type) {
            Type::Array(element) => {
                self.report_if_not_assignable(&index_type, &Type::Number, index.text_range());
                *element
            }
            // The series from element i onward, not element i.
            series if is_series(&series) => {
                self.report_if_not_assignable(&index_type, &Type::Number, index.text_range());
                series_type()
            }
            Type::Object(object) => {
                if let Type::StringLiteral(key) | Type::NumberLiteral(key) = &index_type
                    && object.properties.contains_key(key)
                {
                    return self.member_type(&owner_type, key, index.text_range());
                }
                if !self.is_container_type(&owner_type) {
                    self.check_table_key(&index_type, index.text_range());
                }
                match &object.index {
                    Some((key, value)) => {
                        self.report_if_not_assignable(&index_type, key, index.text_range());
                        value.clone()
                    }
                    None => Type::Any,
                }
            }
            _ => {
                self.check_indexable(&owner_type, owner.text_range());
                Type::Any
            }
        }
    }

    /// `value as Type`, or `call() as (A, B)` for every value of a call.
    /// Returns the cast's type, which is several values for a list cast.
    pub(super) fn check_cast(&mut self, expression: &SyntaxNode) -> Type {
        let (Some(value), Some(target_syntax)) = (first_expression(expression), first_type(expression)) else {
            return Type::Any;
        };
        let values = match value.kind() {
            SyntaxKind::CallExpression => self.check_call(&value),
            SyntaxKind::MethodCallExpression => self.check_method_call(&value),
            SyntaxKind::VarargExpression => Type::Values(Vec::new(), Some(Box::new(self.vararg_type()))),
            _ => self.check_expression(&value, None),
        };
        let is_list = is_list_cast(expression);
        let spreads = matches!(
            value.kind(),
            SyntaxKind::CallExpression | SyntaxKind::MethodCallExpression | SyntaxKind::VarargExpression
        );
        if is_list && !spreads {
            self.sink
                .error(expression.text_range(), "Only a call or '...' can be cast to a list of types.".to_string());
            return Type::Any;
        }
        if !is_list && values.may_have_several_values() && extra_values_are_kept(expression) {
            self.sink.error(expression.text_range(), SPREADING_CAST_MESSAGE.to_string());
        }
        let actual = if is_list { values } else { values.value_at(0) };
        let parameters = self.type_parameters_in_scope();
        let resolver = self.environment.resolver();
        let target = if is_list {
            resolver.return_type(&target_syntax, &parameters, &mut self.sink)
        } else {
            resolver.resolve(&target_syntax, &parameters, &mut self.sink)
        };
        let is_comparable = actual.is_any()
            || target.is_any()
            || matches!(actual, Type::Unknown)
            || self.is_assignable(&actual, &target)
            || self.is_assignable(&target, &actual);
        if !is_comparable {
            self.sink.error(
                expression.text_range(),
                format!("Conversion of type '{actual}' to type '{target}' may be a mistake."),
            );
        }
        target
    }

    fn check_binary(&mut self, expression: &SyntaxNode) -> Type {
        let operands: Vec<SyntaxNode> = expressions(expression).collect();
        let (Some(operator), [left, right]) = (tokens(expression).next(), operands.as_slice()) else {
            return Type::Any;
        };
        match operator.kind() {
            SyntaxKind::AndKeyword | SyntaxKind::OrKeyword => {
                self.check_logical(operator.kind() == SyntaxKind::AndKeyword, left, right)
            }
            SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent => {
                let left_type = self.check_expression(left, None);
                let right_type = self.check_expression(right, None);
                if self.may_be_series(&left_type) || self.may_be_series(&right_type) {
                    self.check_operands(operator.text(), (left, &left_type), (right, &right_type), is_series_operand);
                    return self.series_arithmetic_result(&[&left_type, &right_type]);
                }
                self.check_operands(operator.text(), (left, &left_type), (right, &right_type), is_number);
                Type::Number
            }
            SyntaxKind::Caret => {
                let left_type = self.check_expression(left, None);
                let right_type = self.check_expression(right, None);
                self.check_operands(operator.text(), (left, &left_type), (right, &right_type), is_number);
                Type::Number
            }
            SyntaxKind::DoubleDot => {
                let left_type = self.check_expression(left, None);
                let right_type = self.check_expression(right, None);
                if self.may_be_series(&left_type) || self.may_be_series(&right_type) {
                    self.check_series_concatenation((left, &left_type), (right, &right_type));
                    return Type::String;
                }
                self.check_operands(operator.text(), (left, &left_type), (right, &right_type), |value| {
                    matches!(value, Type::Number | Type::NumberLiteral(_) | Type::String | Type::StringLiteral(_))
                });
                Type::String
            }
            SyntaxKind::Less | SyntaxKind::Greater | SyntaxKind::LessEquals | SyntaxKind::GreaterEquals => {
                let left_type = self.check_expression(left, None);
                let right_type = self.check_expression(right, None);
                let left_type = self.require_not_nil(&left_type, left.text_range());
                let right_type = self.require_not_nil(&right_type, right.text_range());
                self.check_ordering(operator.text(), &left_type, &right_type, expression);
                Type::Boolean
            }
            SyntaxKind::EqualsEquals | SyntaxKind::TildeEquals | SyntaxKind::BangEquals => {
                let left_type = self.check_expression(left, None);
                let right_type = self.check_expression(right, None);
                if !self.types_overlap(&left_type, &right_type) {
                    self.sink.error(
                        expression.text_range(),
                        format!(
                            "This comparison appears to be unintentional because the types '{left_type}' and '{right_type}' have no overlap."
                        ),
                    );
                }
                Type::Boolean
            }
            _ => {
                self.check_expression(left, None);
                self.check_expression(right, None);
                Type::Boolean
            }
        }
    }

    /// Whether a value of `left` can ever equal one of `right`, as
    /// TypeScript decides it for `===`. Comparing with nil is always fine,
    /// and `any` or `unknown` may hold anything.
    fn types_overlap(&self, left: &Type, right: &Type) -> bool {
        let left = self.expand(left);
        let right = self.expand(right);
        let is_open = |value: &Type| value.may_be_nil() || value.members().iter().any(|member| member.is_any());
        if is_open(&left) || is_open(&right) {
            return true;
        }
        left.members().iter().any(|left_member| {
            let left_member = self.expand(left_member);
            right.members().iter().any(|right_member| {
                let right_member = self.expand(right_member);
                matches!(left_member, Type::Unknown)
                    || matches!(right_member, Type::Unknown)
                    || series_may_equal(&left_member, &right_member)
                    || self.constant_may_equal_name(&left_member, &right_member)
                    || self.is_assignable(&left_member, &right_member)
                    || self.is_assignable(&right_member, &left_member)
            })
        })
    }

    /// `a and b` is `b` when `a` is truthy, else `a`; `a or b` is `a` when
    /// truthy, else `b`. `b` is checked with what `a` implies. In a plain
    /// file an `any` operand makes the result `any`, as in TypeScript.
    fn check_logical(&mut self, is_and: bool, left: &SyntaxNode, right: &SyntaxNode) -> Type {
        let left_type = self.check_expression(left, None);
        let implied = self.condition_narrowings(left);
        let before = self.scopes.narrowings();
        for (key, value) in if is_and { &implied.when_true } else { &implied.when_false } {
            self.scopes.narrow(key.clone(), value.clone());
        }
        let right_type = self.check_expression(right, None);
        self.scopes.restore_narrowings(before);
        let holds_any = |value: &Type| value.members().iter().any(Type::is_any);
        if self.rules.lenient && (holds_any(&left_type) || holds_any(&right_type)) {
            return Type::Any;
        }
        let kept_left = if is_and { left_type.falsy_part() } else { left_type.truthy_part() };
        Type::union([kept_left, right_type])
    }

    /// Both operands must be nil-free and satisfy `accepts` (or be `any`).
    fn check_operands(
        &mut self,
        operator: &str,
        (left, left_type): (&SyntaxNode, &Type),
        (right, right_type): (&SyntaxNode, &Type),
        accepts: fn(&Type) -> bool,
    ) {
        let mut reported_nil = false;
        for (operand, value) in [(left, left_type), (right, right_type)] {
            if value.may_be_nil() {
                self.report_possibly_nil(operand.text_range());
                reported_nil = true;
            }
        }
        let fits = |value: &Type| {
            let value = self.expand(&value.without_nil());
            value.members().iter().all(|member| member.is_any() || accepts(&self.expand(member)))
        };
        let operands_fit = fits(left_type) && fits(right_type);
        if !reported_nil && !operands_fit {
            let range = left.text_range().cover(right.text_range());
            self.sink.error(
                range,
                format!("Operator '{operator}' cannot be applied to types '{left_type}' and '{right_type}'."),
            );
        }
    }

    /// `<` and friends compare two numbers or two strings.
    fn check_ordering(&mut self, operator: &str, left: &Type, right: &Type, expression: &SyntaxNode) {
        if left.is_any() || right.is_any() {
            return;
        }
        let fits = if self.may_be_series(left) || self.may_be_series(right) {
            self.is_series_comparand(left) && self.is_series_comparand(right)
        } else {
            let is_number = |value: &Type| self.is_assignable(value, &Type::Number);
            let is_string = |value: &Type| self.is_assignable(value, &Type::String);
            (is_number(left) && is_number(right)) || (is_string(left) && is_string(right))
        };
        if !fits {
            self.sink.error(
                expression.text_range(),
                format!("Operator '{operator}' cannot be applied to types '{left}' and '{right}'."),
            );
        }
    }

    fn check_unary(&mut self, expression: &SyntaxNode) -> Type {
        let (Some(operator), Some(operand)) = (tokens(expression).next(), first_expression(expression)) else {
            return Type::Any;
        };
        let operand_type = self.check_expression(&operand, None);
        match operator.kind() {
            SyntaxKind::NotKeyword => Type::Boolean,
            SyntaxKind::Minus if self.may_be_series(&operand_type) => {
                let value = self.require_not_nil(&operand_type, operand.text_range());
                let fits = self.expand(&value).members().iter().all(|member| {
                    let member = self.expand(member);
                    member.is_any() || is_series(&member) || is_number(&member)
                });
                if !fits {
                    self.sink
                        .error(expression.text_range(), format!("Operator '-' cannot be applied to type '{value}'."));
                }
                self.series_arithmetic_result(&[&value])
            }
            SyntaxKind::Minus => {
                let value = self.require_not_nil(&operand_type, operand.text_range());
                if !value.is_any() && !self.is_assignable(&value, &Type::Number) {
                    self.sink
                        .error(expression.text_range(), format!("Operator '-' cannot be applied to type '{value}'."));
                }
                Type::Number
            }
            SyntaxKind::Hash => {
                let value = self.require_not_nil(&operand_type, operand.text_range());
                // A series has a length; other engine values (enumeration
                // constants, colors) raise an error.
                let has_length = self.expand(&value).members().iter().all(|member| {
                    let member = self.expand(member);
                    is_series(&member)
                        || matches!(
                            member,
                            Type::Any | Type::String | Type::StringLiteral(_) | Type::Array(_) | Type::Object(_)
                        )
                });
                if !has_length {
                    self.sink
                        .error(expression.text_range(), format!("Operator '#' cannot be applied to type '{value}'."));
                }
                Type::Number
            }
            _ => Type::Any,
        }
    }
}
