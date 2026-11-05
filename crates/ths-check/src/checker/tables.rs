//! Table constructors: checked field by field against an expected type,
//! or inferred when nothing is expected.

use std::collections::BTreeMap;
use std::rc::Rc;

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::{Checker, implicit_declared_type};
use crate::syntax_access::{expressions, first_expression, first_name};
use crate::types::{ObjectType, Property, Type};

impl Checker<'_> {
    pub(super) fn check_table(&mut self, table: &SyntaxNode, expected: Option<&Type>) -> Type {
        let target = expected.and_then(|expected| self.table_target(expected));
        match target {
            Some((target, Type::Array(element))) => {
                self.check_array_fields(table, &element);
                target
            }
            Some((target, Type::Object(object))) => {
                self.check_object_fields(table, &target, &object);
                target
            }
            _ => self.infer_table(table),
        }
    }

    /// The one array or object type in `expected` (nil aside), with the
    /// unexpanded type to report and return.
    fn table_target(&self, expected: &Type) -> Option<(Type, Type)> {
        let mut candidates: Vec<(Type, Type)> = Vec::new();
        for member in expected.without_nil().members() {
            match self.expand(member) {
                // A named alias of a union: look at its members in turn.
                Type::Union(inner) => candidates.extend(
                    inner.iter().filter(|part| **part != Type::Nil).map(|part| (part.clone(), self.expand(part))),
                ),
                expanded => candidates.push((member.clone(), expanded)),
            }
        }
        // An inferred table type is no contract to check a new table against.
        candidates.retain(|(_, expanded)| match expanded {
            Type::Array(_) => true,
            Type::Object(object) => !object.open,
            _ => false,
        });
        match candidates.as_slice() {
            [single] => Some(single.clone()),
            _ => None,
        }
    }

    fn check_array_fields(&mut self, table: &SyntaxNode, element: &Type) {
        for field in table.children() {
            match field.kind() {
                SyntaxKind::PositionalField => {
                    if let Some(value) = first_expression(&field) {
                        self.check_assignable_expression(&value, element);
                    }
                }
                _ => {
                    self.sink.error(
                        field.text_range(),
                        format!("A table of type '{element}[]' may only have positional fields."),
                    );
                    self.check_field_values(&field);
                }
            }
        }
    }

    /// Like TypeScript's object literal checks: each field must exist in
    /// the target and fit it, and every required property must be given.
    fn check_object_fields(&mut self, table: &SyntaxNode, target: &Type, object: &ObjectType) {
        let mut given = Vec::new();
        for field in table.children() {
            match field.kind() {
                SyntaxKind::NamedField => {
                    let (Some(name), Some(value)) = (first_name(&field), first_expression(&field)) else {
                        continue;
                    };
                    given.push(name.text().to_string());
                    match object.properties.get(name.text()) {
                        Some(property) => {
                            let accepted = if property.optional {
                                Type::union([property.value.clone(), Type::Nil])
                            } else {
                                property.value.clone()
                            };
                            self.check_assignable_expression(&value, &accepted);
                        }
                        None => match &object.index {
                            Some((_, index_value)) => {
                                self.check_assignable_expression(&value, index_value);
                            }
                            None => {
                                self.sink.error(
                                    name.text_range(),
                                    format!(
                                        "Object literal may only specify known properties, and '{}' does not exist in type '{target}'.",
                                        name.text()
                                    ),
                                );
                                self.check_expression(&value, None);
                            }
                        },
                    }
                }
                SyntaxKind::PositionalField | SyntaxKind::IndexedField => match &object.index {
                    Some((_, index_value)) => {
                        if let Some(value) = expressions(&field).last() {
                            self.check_assignable_expression(&value, index_value);
                        }
                    }
                    None => {
                        self.sink.error(
                            field.text_range(),
                            format!("Type '{target}' has no index signature for this field."),
                        );
                        self.check_field_values(&field);
                    }
                },
                _ => {}
            }
        }
        for (name, property) in &object.properties {
            if !property.optional && !given.contains(name) {
                self.sink.error(
                    table.text_range(),
                    format!("Property '{name}' is missing but required in type '{target}'."),
                );
            }
        }
    }

    fn check_field_values(&mut self, field: &SyntaxNode) {
        for value in expressions(field) {
            self.check_expression(&value, None);
        }
    }

    /// Only positional fields: an array of their widened types. Otherwise
    /// an object of the named fields, with a number index for the rest.
    fn infer_table(&mut self, table: &SyntaxNode) -> Type {
        let mut properties = BTreeMap::new();
        let mut positional = Vec::new();
        let mut has_indexed_fields = false;
        for field in table.children() {
            match field.kind() {
                SyntaxKind::NamedField => {
                    let (Some(name), Some(value)) = (first_name(&field), first_expression(&field)) else {
                        continue;
                    };
                    let initial = self.check_expression(&value, None).widened();
                    let value = implicit_declared_type(&initial);
                    properties.insert(name.text().to_string(), Property { value, optional: false });
                }
                SyntaxKind::PositionalField => {
                    if let Some(value) = first_expression(&field) {
                        positional.push(self.check_expression(&value, None).widened());
                    }
                }
                SyntaxKind::IndexedField => {
                    has_indexed_fields = true;
                    self.check_field_values(&field);
                }
                _ => {}
            }
        }
        if properties.is_empty() && !has_indexed_fields && !positional.is_empty() {
            return Type::Array(Box::new(Type::union(positional)));
        }
        let index = if has_indexed_fields {
            Some((Type::Any, Type::Any))
        } else if !positional.is_empty() {
            Some((Type::Number, Type::union(positional)))
        } else {
            None
        };
        Type::Object(Rc::new(ObjectType { properties, index, open: true }))
    }
}
