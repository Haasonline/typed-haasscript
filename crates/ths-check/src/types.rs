//! The types the checker reasons about, and the operations on a single type
//! that need no declarations: building unions, truthiness, widening and
//! display.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use crate::primitive_type_names;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Unchecked. Unlike TypeScript's `any`, it does not include nil.
    Any,
    /// Anything, including nil; must be narrowed or cast before use.
    Unknown,
    Never,
    Nil,
    Boolean,
    Number,
    String,
    BooleanLiteral(bool),
    /// The literal's source text, normalized (`1.0` and `1` are both `1`).
    NumberLiteral(String),
    StringLiteral(String),
    Array(Box<Type>),
    Object(Rc<ObjectType>),
    Function(Rc<FunctionType>),
    /// Two or more members, flattened, without duplicates. Build with
    /// [`Type::union`].
    Union(Vec<Type>),
    /// A named `type` alias or `interface` with its type arguments
    /// (`Box<number>`), expanded on demand so that recursive declarations
    /// stay finite.
    Reference(String, Vec<Type>),
    /// A type parameter (`T`) inside the generic declaration that declares
    /// it: opaque, usable as its constraint, and only `T` itself (or `any`
    /// or `never`) is assignable to it.
    Parameter(Rc<TypeParameter>),
    /// Several values, `(number, string)`, then optionally any number more
    /// of the rest type, `(number, ...string)`: only as a function's return
    /// type, and either two or more fixed values or a rest type. A call used
    /// as one value gives the first.
    Values(Vec<Type>, Option<Box<Type>>),
    /// A type known only by its name, from `declare type Name extends
    /// Base`: assignable only to itself and the types it extends.
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: String,
    /// `T extends C`
    pub constraint: Option<Type>,
    /// `T = D`
    pub default: Option<Type>,
}

/// Type arguments by type parameter name.
pub type Substitution = HashMap<String, Type>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectType {
    pub properties: BTreeMap<String, Property>,
    /// `[key: K]: V`
    pub index: Option<(Type, Type)>,
    /// Inferred from a table constructor rather than declared. Lua builds
    /// tables field by field (`local t = {}` then `t.price = 1`), so, as in
    /// TypeScript's checking of JavaScript, unknown fields of an open table
    /// are `any` instead of an error.
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub value: Type,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    /// `<T, U>`: inferred from the arguments at each call.
    pub type_parameters: Vec<Rc<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    /// The type of each `...` argument, if the function takes them.
    pub rest: Option<Type>,
    pub returns: Type,
    /// A TradeServer command (a `declare function`): it converts any value
    /// passed for a string parameter to text, where a Lua function would
    /// receive the value itself.
    pub is_command: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub value: Type,
    pub optional: bool,
    /// No annotation and no contextual type: inside the function it is an
    /// inferred `any` that accepts nil, not an optional `any | nil`.
    pub implicit: bool,
}

impl Type {
    /// The union of `members`: flattened, duplicates and `never` removed,
    /// literals dropped when their base type is present. One member is
    /// returned as itself; none gives `never`.
    pub fn union(members: impl IntoIterator<Item = Type>) -> Type {
        let mut flat: Vec<Type> = Vec::new();
        for member in members {
            match member {
                Type::Union(inner) => flat.extend(inner),
                Type::Never => {}
                other => flat.push(other),
            }
        }
        if flat.contains(&Type::Unknown) {
            return Type::Unknown;
        }
        let snapshot = flat.clone();
        let mut kept: Vec<Type> = Vec::new();
        for member in flat {
            let base_is_present = member.literal_base().is_some_and(|base| snapshot.contains(&base));
            if !base_is_present && !kept.contains(&member) {
                kept.push(member);
            }
        }
        match kept.len() {
            0 => Type::Never,
            1 => kept.pop().expect("one member"),
            _ => Type::Union(kept),
        }
    }

    pub fn members(&self) -> &[Type] {
        match self {
            Type::Union(members) => members,
            single => std::slice::from_ref(single),
        }
    }

    fn literal_base(&self) -> Option<Type> {
        match self {
            Type::BooleanLiteral(_) => Some(Type::Boolean),
            Type::NumberLiteral(_) => Some(Type::Number),
            Type::StringLiteral(_) => Some(Type::String),
            _ => None,
        }
    }

    /// Literal types become their base type, as when a literal initializes a
    /// mutable local: `local x = 1` is a `number`.
    pub fn widened(&self) -> Type {
        match self {
            Type::Union(members) => Type::union(members.iter().map(Type::widened)),
            other => other.literal_base().unwrap_or_else(|| other.clone()),
        }
    }

    pub fn may_be_nil(&self) -> bool {
        self.members().iter().any(|member| matches!(member, Type::Nil | Type::Unknown))
    }

    pub fn without_nil(&self) -> Type {
        match self {
            Type::Unknown => Type::Any,
            other => Type::union(other.members().iter().filter(|member| **member != Type::Nil).cloned()),
        }
    }

    /// What remains when the value is known to be truthy: not nil, not false.
    pub fn truthy_part(&self) -> Type {
        Type::union(self.members().iter().filter_map(|member| match member {
            Type::Nil | Type::BooleanLiteral(false) => None,
            Type::Boolean => Some(Type::BooleanLiteral(true)),
            Type::Unknown => Some(Type::Any),
            other => Some(other.clone()),
        }))
    }

    /// What remains when the value is known to be falsy: nil or false.
    pub fn falsy_part(&self) -> Type {
        Type::union(self.members().iter().filter_map(|member| match member {
            Type::Nil => Some(Type::Nil),
            Type::Boolean | Type::BooleanLiteral(false) => Some(Type::BooleanLiteral(false)),
            Type::Any | Type::Unknown => Some(Type::union([Type::Nil, Type::BooleanLiteral(false)])),
            _ => None,
        }))
    }

    /// The value at `index` of what a call returns: past the end of a
    /// value list, or past the first of a single value, Lua supplies nil.
    /// A rest value may be absent, so it may be nil.
    pub fn value_at(&self, index: usize) -> Type {
        match self {
            Type::Values(values, rest) => match (values.get(index), rest) {
                (Some(value), _) => value.clone(),
                (None, Some(rest)) => Type::union([(**rest).clone(), Type::Nil]),
                (None, None) => Type::Nil,
            },
            single if index == 0 => single.clone(),
            _ => Type::Nil,
        }
    }

    /// The type a function declared with this return type must return at
    /// `index`, if it may return a value there.
    pub fn declared_value_at(&self, index: usize) -> Option<Type> {
        match self {
            Type::Values(values, rest) => values.get(index).cloned().or_else(|| rest.as_deref().cloned()),
            single => (index == 0).then(|| single.clone()),
        }
    }

    /// How many values come before any rest values.
    pub fn fixed_value_count(&self) -> usize {
        match self {
            Type::Values(values, _) => values.len(),
            _ => 1,
        }
    }

    pub fn has_rest_values(&self) -> bool {
        matches!(self, Type::Values(_, Some(_)))
    }

    pub fn may_have_several_values(&self) -> bool {
        self.fixed_value_count() > 1 || self.has_rest_values()
    }

    pub fn is_any(&self) -> bool {
        *self == Type::Any
    }

    /// Replaces type parameters named in `substitution`. A function type's
    /// own type parameters shadow outer ones of the same name.
    pub fn substituted(&self, substitution: &Substitution) -> Type {
        if substitution.is_empty() {
            return self.clone();
        }
        match self {
            Type::Parameter(parameter) => substitution.get(&parameter.name).cloned().unwrap_or_else(|| self.clone()),
            Type::Array(element) => Type::Array(Box::new(element.substituted(substitution))),
            Type::Values(values, rest) => Type::Values(
                values.iter().map(|value| value.substituted(substitution)).collect(),
                rest.as_ref().map(|rest| Box::new(rest.substituted(substitution))),
            ),
            Type::Union(members) => Type::union(members.iter().map(|member| member.substituted(substitution))),
            Type::Reference(name, arguments) => Type::Reference(
                name.clone(),
                arguments.iter().map(|argument| argument.substituted(substitution)).collect(),
            ),
            Type::Object(object) => Type::Object(Rc::new(ObjectType {
                properties: object
                    .properties
                    .iter()
                    .map(|(name, property)| {
                        let value = property.value.substituted(substitution);
                        (name.clone(), Property { value, optional: property.optional })
                    })
                    .collect(),
                index: object
                    .index
                    .as_ref()
                    .map(|(key, value)| (key.substituted(substitution), value.substituted(substitution))),
                open: object.open,
            })),
            Type::Function(function) => {
                let mut inner = substitution.clone();
                for own in &function.type_parameters {
                    inner.remove(&own.name);
                }
                Type::Function(Rc::new(function.substituted(&inner)))
            }
            _ => self.clone(),
        }
    }
}

/// Type names by the names they become.
pub type Renaming = HashMap<String, String>;

impl Type {
    /// Renames the named types `renaming` lists, wherever they appear: a
    /// module's own types take the names they have in the files that
    /// import it.
    pub fn renamed(&self, renaming: &Renaming) -> Type {
        if renaming.is_empty() {
            return self.clone();
        }
        let rename = |name: &String| renaming.get(name).cloned().unwrap_or_else(|| name.clone());
        match self {
            Type::Reference(name, arguments) => {
                Type::Reference(rename(name), arguments.iter().map(|argument| argument.renamed(renaming)).collect())
            }
            Type::Opaque(name) => Type::Opaque(rename(name)),
            Type::Array(element) => Type::Array(Box::new(element.renamed(renaming))),
            Type::Union(members) => Type::union(members.iter().map(|member| member.renamed(renaming))),
            Type::Values(values, rest) => Type::Values(
                values.iter().map(|value| value.renamed(renaming)).collect(),
                rest.as_ref().map(|rest| Box::new(rest.renamed(renaming))),
            ),
            Type::Object(object) => Type::Object(Rc::new(object.renamed(renaming))),
            Type::Function(function) => Type::Function(Rc::new(function.renamed(renaming))),
            Type::Parameter(parameter) => Type::Parameter(Rc::new(parameter.renamed(renaming))),
            _ => self.clone(),
        }
    }
}

impl ObjectType {
    pub(crate) fn renamed(&self, renaming: &Renaming) -> ObjectType {
        ObjectType {
            properties: self
                .properties
                .iter()
                .map(|(name, property)| {
                    (name.clone(), Property { value: property.value.renamed(renaming), optional: property.optional })
                })
                .collect(),
            index: self.index.as_ref().map(|(key, value)| (key.renamed(renaming), value.renamed(renaming))),
            open: self.open,
        }
    }
}

impl TypeParameter {
    pub(crate) fn renamed(&self, renaming: &Renaming) -> TypeParameter {
        TypeParameter {
            name: self.name.clone(),
            constraint: self.constraint.as_ref().map(|constraint| constraint.renamed(renaming)),
            default: self.default.as_ref().map(|default| default.renamed(renaming)),
        }
    }
}

impl FunctionType {
    pub(crate) fn substituted(&self, substitution: &Substitution) -> FunctionType {
        FunctionType {
            type_parameters: self.type_parameters.clone(),
            parameters: self
                .parameters
                .iter()
                .map(|parameter| Parameter { value: parameter.value.substituted(substitution), ..parameter.clone() })
                .collect(),
            rest: self.rest.as_ref().map(|rest| rest.substituted(substitution)),
            returns: self.returns.substituted(substitution),
            is_command: self.is_command,
        }
    }

    pub(crate) fn renamed(&self, renaming: &Renaming) -> FunctionType {
        FunctionType {
            type_parameters: self
                .type_parameters
                .iter()
                .map(|parameter| Rc::new(parameter.renamed(renaming)))
                .collect(),
            parameters: self
                .parameters
                .iter()
                .map(|parameter| Parameter { value: parameter.value.renamed(renaming), ..parameter.clone() })
                .collect(),
            rest: self.rest.as_ref().map(|rest| rest.renamed(renaming)),
            returns: self.returns.renamed(renaming),
            is_command: self.is_command,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Any => formatter.write_str(primitive_type_names::ANY),
            Type::Unknown => formatter.write_str(primitive_type_names::UNKNOWN),
            Type::Never => formatter.write_str(primitive_type_names::NEVER),
            Type::Nil => formatter.write_str(primitive_type_names::NIL),
            Type::Boolean => formatter.write_str(primitive_type_names::BOOLEAN),
            Type::Number => formatter.write_str(primitive_type_names::NUMBER),
            Type::String => formatter.write_str(primitive_type_names::STRING),
            Type::BooleanLiteral(value) => write!(formatter, "{value}"),
            Type::NumberLiteral(text) => formatter.write_str(text),
            Type::StringLiteral(text) => write!(formatter, "\"{text}\""),
            Type::Array(element) => match **element {
                Type::Union(_) | Type::Function(_) => write!(formatter, "({element})[]"),
                _ => write!(formatter, "{element}[]"),
            },
            Type::Object(object) => fmt::Display::fmt(object, formatter),
            Type::Function(function) => fmt::Display::fmt(function, formatter),
            Type::Union(members) => {
                let texts: Vec<String> = members.iter().map(ToString::to_string).collect();
                formatter.write_str(&texts.join(" | "))
            }
            Type::Reference(name, arguments) if arguments.is_empty() => formatter.write_str(name),
            Type::Reference(name, arguments) => {
                let texts: Vec<String> = arguments.iter().map(ToString::to_string).collect();
                write!(formatter, "{name}<{}>", texts.join(", "))
            }
            Type::Parameter(parameter) => formatter.write_str(&parameter.name),
            Type::Opaque(name) => formatter.write_str(name),
            Type::Values(values, rest) => {
                let mut texts: Vec<String> = values.iter().map(ToString::to_string).collect();
                if let Some(rest) = rest {
                    texts.push(format!("...{rest}"));
                }
                match texts.as_slice() {
                    [only] if values.is_empty() => formatter.write_str(only),
                    _ => write!(formatter, "({})", texts.join(", ")),
                }
            }
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut members: Vec<String> = self
            .properties
            .iter()
            .map(|(name, property)| {
                let marker = if property.optional { "?" } else { "" };
                format!("{name}{marker}: {}", property.value)
            })
            .collect();
        if let Some((key, value)) = &self.index {
            members.push(format!("[key: {key}]: {value}"));
        }
        if members.is_empty() {
            return formatter.write_str("{}");
        }
        write!(formatter, "{{ {} }}", members.join("; "))
    }
}

impl fmt::Display for FunctionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parameters: Vec<String> = self
            .parameters
            .iter()
            .map(|parameter| {
                let marker = if parameter.optional { "?" } else { "" };
                format!("{}{marker}: {}", parameter.name, parameter.value)
            })
            .collect();
        if let Some(rest) = &self.rest {
            parameters.push(format!("...: {rest}"));
        }
        if !self.type_parameters.is_empty() {
            let names: Vec<&str> = self.type_parameters.iter().map(|parameter| parameter.name.as_str()).collect();
            write!(formatter, "<{}>", names.join(", "))?;
        }
        write!(formatter, "({}) => {}", parameters.join(", "), self.returns)
    }
}
