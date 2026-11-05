//! What `DefineParameter(kind, name, description, isRequired, default)` gives a
//! custom command: the argument a caller passed, converted to the parameter's
//! type, or the default unchanged when the caller left it out and on the compile
//! pass (verified on the TradeServer): a `NumberType` parameter receives a
//! number, a `StringType` one text, a `BooleanType` one a boolean, an `EnumType`
//! one the constant (of any category, though comparing it with a constant of
//! another category stops the script, so it is taken to be of the default's
//! category), a `ListNumberType` one a series (a Lua table argument is
//! converted) or its default, and `DynamicType` and `ListDynamicType` ones
//! whatever is passed. A nil default does not compile, so the value is never
//! nil.

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use crate::command_signature::{DEFAULT_VALUE_POSITION, DEFINE_PARAMETER, ParameterKind, constant_kind};
use crate::engine_types::{ENUM_TYPE, SERIES_TYPE};
use crate::enum_categories::category_of;
use crate::syntax_access::{call_arguments, first_name};
use crate::types::Type;

impl Checker<'_> {
    /// The kind of the parameter `call` defines, when it calls the built-in
    /// `DefineParameter` with one of the type constants.
    pub(super) fn defined_parameter_kind(&self, callee: &SyntaxNode, call: &SyntaxNode) -> Option<ParameterKind> {
        if !self.names_built_in(callee, DEFINE_PARAMETER) {
            return None;
        }
        let kind_argument = call_arguments(call).into_iter().next()?;
        let kind = constant_kind(&kind_argument)?;
        let constant = first_name(&kind_argument)?;
        self.names_built_in(&kind_argument, constant.text()).then_some(kind)
    }

    /// Checks a `DefineParameter` call of `kind` as any call, and gives
    /// what the parameter receives.
    pub(super) fn check_define_parameter(
        &mut self,
        kind: ParameterKind,
        call: &SyntaxNode,
        checked: impl FnOnce(&mut Self) -> Type,
    ) -> Type {
        let default = call_arguments(call).get(DEFAULT_VALUE_POSITION).map(SyntaxNode::text_range);
        if let Some(range) = default {
            self.parameter_defaults.want(range);
        }
        let result = checked(self);
        let default_type = default.and_then(|range| self.parameter_defaults.take(range));
        self.received_type(kind, default_type).unwrap_or(result)
    }

    /// What a parameter of `kind` receives; `None` when it can be anything.
    fn received_type(&self, kind: ParameterKind, default: Option<Type>) -> Option<Type> {
        let series = Type::Opaque(SERIES_TYPE.to_string());
        let received = match kind {
            ParameterKind::Number => Type::Number,
            ParameterKind::String => Type::String,
            ParameterKind::Boolean => Type::Boolean,
            ParameterKind::Enum => default
                .and_then(|default| category_of(self.environment, &default))
                .unwrap_or_else(|| Type::Opaque(ENUM_TYPE.to_string())),
            ParameterKind::ListNumber => {
                let list = Type::union([series.clone(), Type::Array(Box::new(Type::Number))]);
                match default.map(|default| self.opaque_members_expanded(&default.widened())) {
                    Some(default) if !default.is_any() && self.is_assignable(&default, &list) => {
                        Type::union([series, default])
                    }
                    _ => list,
                }
            }
            ParameterKind::Dynamic | ParameterKind::ListDynamic | ParameterKind::Void => return None,
        };
        Some(received)
    }

    /// `value` with each member naming an opaque type replaced by it, so a
    /// union with the series written either way holds it once.
    fn opaque_members_expanded(&self, value: &Type) -> Type {
        Type::union(value.members().iter().map(|member| match self.expand(member) {
            opaque @ Type::Opaque(_) => opaque,
            _ => member.clone(),
        }))
    }

    /// Whether `expression` is the name `name` and reads the declared
    /// global, not a local of the same name.
    fn names_built_in(&self, expression: &SyntaxNode, name: &str) -> bool {
        expression.kind() == SyntaxKind::NameExpression
            && first_name(expression).is_some_and(|read| read.text() == name)
            && self.scopes.lookup_declared_global(name).is_some_and(|global| self.scopes.lookup(name) == Some(global))
    }
}
