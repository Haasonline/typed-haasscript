//! Whether a value of one type may be used where another is expected,
//! following TypeScript's structural rules except that nil is never
//! implicitly allowed (see `docs/design.md`).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use crate::type_environment::TypeEnvironment;
use crate::types::{FunctionType, ObjectType, Substitution, Type};

/// How many references a comparison may expand inside one another before it
/// assumes the types fit, as TypeScript does: a type that grows each time it
/// expands (`Nest<T> { inner: Nest<Nest<T>> }`) never repeats a pair.
const NESTED_EXPANSION_LIMIT: usize = 32;

pub(crate) struct Assignability<'environment> {
    environment: &'environment TypeEnvironment,
    /// Pairs with a reference on either side being compared further up the
    /// stack, as displayed (`List<number>`, `List<number> | nil`). Meeting
    /// one again means a recursive type compared with itself: assume it holds.
    in_progress: RefCell<HashSet<(String, String)>>,
    /// How many reference expansions enclose the current comparison.
    expansion_depth: Cell<usize>,
}

impl<'environment> Assignability<'environment> {
    pub(crate) fn new(environment: &'environment TypeEnvironment) -> Self {
        Assignability { environment, in_progress: RefCell::new(HashSet::new()), expansion_depth: Cell::new(0) }
    }

    pub(crate) fn is_assignable(&self, source: &Type, target: &Type) -> bool {
        if source == target {
            return true;
        }
        if matches!(source, Type::Reference(..)) || matches!(target, Type::Reference(..)) {
            return self.is_assignable_expanded(source, target);
        }
        match (source, target) {
            (Type::Never, _) | (_, Type::Unknown) => true,
            (Type::Union(members), _) => members.iter().all(|member| self.is_assignable(member, target)),
            // `any` accepts and fits everything except nil.
            (Type::Nil, Type::Any) => false,
            (Type::Any, _) => !matches!(target, Type::Nil | Type::Never),
            (_, Type::Any) => true,
            // Values compare position by position, a missing one as nil, then
            // rest values with rest values; where one value is expected, the
            // first is used.
            (Type::Values(..), target) if !matches!(target, Type::Values(..)) => {
                self.is_assignable(&source.value_at(0), target)
            }
            (_, Type::Values(_, target_rest)) => {
                let count = source.fixed_value_count().max(target.fixed_value_count());
                let positions_fit =
                    (0..count).all(|index| self.is_assignable(&source.value_at(index), &target.value_at(index)));
                let rest_fits = match (source, target_rest) {
                    (Type::Values(_, Some(source_rest)), Some(target_rest)) => {
                        self.is_assignable(source_rest, target_rest)
                    }
                    _ => true,
                };
                positions_fit && rest_fits
            }
            // `T` fits wherever its constraint does; nothing but `T` itself
            // (or `any`, `never`) fits `T`, whatever it is instantiated with.
            (Type::Parameter(parameter), _) => {
                target.members().contains(source)
                    || parameter.constraint.as_ref().is_some_and(|constraint| self.is_assignable(constraint, target))
            }
            (_, Type::Parameter(_)) => false,
            (Type::Opaque(name), Type::Opaque(target_name)) => self.environment.opaque_extends(name, target_name),
            (_, Type::Union(members)) => members.iter().any(|member| self.is_assignable(source, member)),
            (Type::BooleanLiteral(_), Type::Boolean)
            | (Type::NumberLiteral(_), Type::Number)
            | (Type::StringLiteral(_), Type::String) => true,
            (Type::Array(source_element), Type::Array(target_element)) => {
                self.is_assignable(source_element, target_element)
            }
            (Type::Object(object), Type::Array(element)) => self.object_fits_array(object, element),
            (Type::Array(element), Type::Object(object)) => self.array_fits_object(element, object),
            (Type::Object(source_object), Type::Object(target_object)) => {
                self.object_fits_object(source_object, target_object)
            }
            (Type::Function(source_function), Type::Function(target_function)) => {
                self.function_fits_function(source_function, target_function)
            }
            _ => false,
        }
    }

    /// Compares with the references on either side expanded, assuming the
    /// fit when the same pair is already being compared further up the stack
    /// or the expansions nest too deep.
    fn is_assignable_expanded(&self, source: &Type, target: &Type) -> bool {
        let depth = self.expansion_depth.get();
        if depth >= NESTED_EXPANSION_LIMIT {
            return true;
        }
        let pair = (source.to_string(), target.to_string());
        if !self.in_progress.borrow_mut().insert(pair.clone()) {
            return true;
        }
        self.expansion_depth.set(depth + 1);
        let result = self.is_assignable(&self.environment.expand(source), &self.environment.expand(target));
        self.expansion_depth.set(depth);
        self.in_progress.borrow_mut().remove(&pair);
        result
    }

    /// A table with no named fields fits any array (`local xs: number[] = {}`).
    fn object_fits_array(&self, object: &ObjectType, element: &Type) -> bool {
        object.properties.is_empty()
            && object.index.as_ref().is_none_or(|(_, value)| self.is_assignable(value, element))
    }

    /// An array fits an object type only through a number index signature.
    fn array_fits_object(&self, element: &Type, object: &ObjectType) -> bool {
        object.properties.values().all(|property| property.optional)
            && object
                .index
                .as_ref()
                .is_none_or(|(key, value)| self.is_assignable(&Type::Number, key) && self.is_assignable(element, value))
    }

    fn object_fits_object(&self, source: &ObjectType, target: &ObjectType) -> bool {
        let properties_fit = target.properties.iter().all(|(name, wanted)| match source.properties.get(name) {
            Some(present) => {
                let present_value = if present.optional {
                    Type::union([present.value.clone(), Type::Nil])
                } else {
                    present.value.clone()
                };
                let wanted_value =
                    if wanted.optional { Type::union([wanted.value.clone(), Type::Nil]) } else { wanted.value.clone() };
                self.is_assignable(&present_value, &wanted_value)
            }
            None => wanted.optional,
        });
        let index_fits = target.index.as_ref().is_none_or(|(_, value)| {
            source.properties.values().all(|property| self.is_assignable(&property.value, value))
                && source.index.as_ref().is_none_or(|(_, source_value)| self.is_assignable(source_value, value))
        });
        properties_fit && index_fits
    }

    /// Parameters compare in the opposite direction (a function accepting
    /// more fits where less is passed); a function may ignore trailing
    /// arguments, so it may declare fewer parameters.
    fn function_fits_function(&self, source: &FunctionType, target: &FunctionType) -> bool {
        if !source.type_parameters.is_empty() {
            return self.function_fits_function(&instantiated_with_constraints(source), target);
        }
        if source.parameters.len() > target.parameters.len() && target.rest.is_none() {
            let extra_are_optional =
                source.parameters[target.parameters.len()..].iter().all(|parameter| parameter.optional);
            if !extra_are_optional {
                return false;
            }
        }
        let parameters_fit =
            source.parameters.iter().zip(&target.parameters).all(|(source_parameter, target_parameter)| {
                self.is_assignable(&target_parameter.value, &source_parameter.value)
            });
        parameters_fit && self.is_assignable(&source.returns, &target.returns)
    }
}

/// A generic function with each type parameter replaced by its constraint,
/// or `any`: a simplification of TypeScript, which infers them from the
/// target signature.
fn instantiated_with_constraints(function: &FunctionType) -> FunctionType {
    let substitution: Substitution = function
        .type_parameters
        .iter()
        .map(|parameter| {
            let bound = parameter.constraint.clone().unwrap_or(Type::Any);
            (parameter.name.clone(), bound)
        })
        .collect();
    let mut instantiated = function.substituted(&substitution);
    instantiated.type_parameters.clear();
    instantiated
}
