//! Inferring a generic function's type arguments from the types of the
//! arguments passed to it.

use std::collections::HashMap;
use std::rc::Rc;

use crate::type_environment::TypeEnvironment;
use crate::types::{Substitution, Type, TypeParameter};

/// How deep inference follows the structure of a parameter type; bounds
/// the walk through recursive declarations such as `List<T>`.
const DEPTH_LIMIT: usize = 8;

/// Candidate types for the type parameters of one call.
pub(crate) struct Inference<'environment> {
    environment: &'environment TypeEnvironment,
    parameters: Vec<Rc<TypeParameter>>,
    candidates: HashMap<String, Vec<Type>>,
}

impl<'environment> Inference<'environment> {
    pub(crate) fn new(environment: &'environment TypeEnvironment, parameters: &[Rc<TypeParameter>]) -> Self {
        Inference { environment, parameters: parameters.to_vec(), candidates: HashMap::new() }
    }

    /// Whether `value` mentions any of the type parameters being inferred.
    pub(crate) fn mentions_parameters(&self, value: &Type) -> bool {
        self.mentions(value, DEPTH_LIMIT)
    }

    fn mentions(&self, value: &Type, depth: usize) -> bool {
        let Some(depth) = depth.checked_sub(1) else {
            return false;
        };
        match value {
            Type::Parameter(parameter) => self.is_own(parameter),
            Type::Array(element) => self.mentions(element, depth),
            Type::Values(values, rest) => {
                values.iter().any(|value| self.mentions(value, depth))
                    || rest.as_ref().is_some_and(|rest| self.mentions(rest, depth))
            }
            Type::Union(members) => members.iter().any(|member| self.mentions(member, depth)),
            Type::Reference(_, arguments) => arguments.iter().any(|argument| self.mentions(argument, depth)),
            Type::Object(object) => {
                object.properties.values().any(|property| self.mentions(&property.value, depth))
                    || object
                        .index
                        .as_ref()
                        .is_some_and(|(key, value)| self.mentions(key, depth) || self.mentions(value, depth))
            }
            Type::Function(function) => {
                function.parameters.iter().any(|parameter| self.mentions(&parameter.value, depth))
                    || function.rest.as_ref().is_some_and(|rest| self.mentions(rest, depth))
                    || self.mentions(&function.returns, depth)
            }
            _ => false,
        }
    }

    fn is_own(&self, parameter: &TypeParameter) -> bool {
        self.parameters.iter().any(|own| own.name == parameter.name)
    }

    /// Records what `argument` says about the type parameters in
    /// `parameter`, walking both types' structure in step.
    pub(crate) fn infer(&mut self, parameter: &Type, argument: &Type) {
        self.infer_within(parameter, argument, DEPTH_LIMIT);
    }

    fn infer_within(&mut self, parameter: &Type, argument: &Type, depth: usize) {
        let Some(depth) = depth.checked_sub(1) else {
            return;
        };
        if !self.mentions(parameter, DEPTH_LIMIT) {
            return;
        }
        match (parameter, argument) {
            (Type::Values(values, rest), _) => {
                for (index, value) in values.iter().enumerate() {
                    self.infer_within(value, &argument.value_at(index), depth);
                }
                if let (Some(rest), Type::Values(_, Some(argument_rest))) = (rest, argument) {
                    self.infer_within(rest, argument_rest, depth);
                }
            }
            (_, Type::Values(..)) => self.infer_within(parameter, &argument.value_at(0), depth),
            (Type::Parameter(own), _) => {
                self.candidates.entry(own.name.clone()).or_default().push(argument.clone());
            }
            (Type::Union(members), _) => self.infer_from_union(members, argument, depth),
            (Type::Reference(name, arguments), Type::Reference(argument_name, argument_arguments))
                if name == argument_name =>
            {
                for (inner, argument_inner) in arguments.iter().zip(argument_arguments) {
                    self.infer_within(inner, argument_inner, depth);
                }
            }
            (Type::Reference(..), _) | (_, Type::Reference(..)) => {
                let parameter = self.environment.expand(parameter);
                let argument = self.environment.expand(argument);
                self.infer_within(&parameter, &argument, depth);
            }
            (Type::Array(element), Type::Array(argument_element)) => {
                self.infer_within(element, argument_element, depth);
            }
            (Type::Object(object), Type::Object(argument_object)) => {
                for (name, property) in &object.properties {
                    if let Some(argument_property) = argument_object.properties.get(name) {
                        self.infer_within(&property.value, &argument_property.value, depth);
                    }
                }
                if let (Some((key, value)), Some((argument_key, argument_value))) =
                    (&object.index, &argument_object.index)
                {
                    self.infer_within(key, argument_key, depth);
                    self.infer_within(value, argument_value, depth);
                }
            }
            (Type::Function(function), Type::Function(argument_function)) => {
                for (inner, argument_inner) in function.parameters.iter().zip(&argument_function.parameters) {
                    self.infer_within(&inner.value, &argument_inner.value, depth);
                }
                self.infer_within(&function.returns, &argument_function.returns, depth);
            }
            _ => {}
        }
    }

    /// `T | nil` given `number | nil`: the argument's members that fit the
    /// union's fixed members are left out, and the rest go to its one
    /// generic member. A union with several generic members is not inferred
    /// from.
    fn infer_from_union(&mut self, members: &[Type], argument: &Type, depth: usize) {
        let (generic, fixed): (Vec<&Type>, Vec<&Type>) =
            members.iter().partition(|member| self.mentions(member, DEPTH_LIMIT));
        let [generic] = generic.as_slice() else {
            return;
        };
        let remaining = Type::union(argument.members().iter().filter(|part| !fixed.contains(part)).cloned());
        if remaining != Type::Never {
            self.infer_within(generic, &remaining, depth);
        }
    }

    /// The type arguments inferred so far: the union of each type
    /// parameter's candidates. An `any` candidate says nothing (a callback
    /// parameter typed before its type parameter was known), so it counts
    /// only when there is no other. A type parameter without candidates
    /// takes its default, else `fallback`.
    pub(crate) fn substitution(&self, fallback: &Type) -> Substitution {
        let mut substitution = Substitution::new();
        for parameter in &self.parameters {
            let candidates = self.candidates.get(&parameter.name);
            let informative: Vec<Type> =
                candidates.into_iter().flatten().filter(|candidate| !candidate.is_any()).cloned().collect();
            let inferred = match candidates {
                Some(_) if !informative.is_empty() => Type::union(informative),
                Some(_) => Type::Any,
                None => parameter
                    .default
                    .as_ref()
                    .map_or_else(|| fallback.clone(), |default| default.substituted(&substitution)),
            };
            substitution.insert(parameter.name.clone(), inferred);
        }
        substitution
    }

    pub(crate) fn parameters(&self) -> &[Rc<TypeParameter>] {
        &self.parameters
    }
}
