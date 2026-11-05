//! Changing the type of an assignment target whose type was inferred: a
//! variable without an annotation, or a field of a table whose type was
//! inferred from its constructor. Later reads see the new type.

use std::rc::Rc;

use ths_syntax::{SyntaxKind, SyntaxNode};

use super::Checker;
use crate::scopes::NarrowingKey;
use crate::syntax_access::{first_expression, first_name, tokens};
use crate::types::{ObjectType, Type};

/// The new declared type of an inferred target, from its current declared
/// type and the assigned value, or `None` to keep it.
pub(super) type Retype = fn(&Checker<'_>, &Type, &Type) -> Option<Type>;

impl Checker<'_> {
    /// Gives the inferred variable or field `target` the type `retype`
    /// returns for `assigned`, and returns that type.
    pub(super) fn retype_inferred(&mut self, target: &SyntaxNode, assigned: &Type, retype: Retype) -> Option<Type> {
        match target.kind() {
            SyntaxKind::NameExpression => self.retype_variable(target, assigned, retype),
            SyntaxKind::FieldExpression => self.retype_field(target, assigned, retype),
            _ => None,
        }
    }

    fn retype_variable(&mut self, target: &SyntaxNode, assigned: &Type, retype: Retype) -> Option<Type> {
        let id = self.scopes.lookup(first_name(target)?.text())?;
        if !self.scopes.is_implicit(id) {
            return None;
        }
        let retyped = retype(self, self.scopes.declared_type(id), assigned)?;
        self.scopes.redeclare(id, retyped.clone());
        Some(retyped)
    }

    /// The new field type is stored in the variable the table is reached
    /// from, so later reads see it.
    fn retype_field(&mut self, target: &SyntaxNode, assigned: &Type, retype: Retype) -> Option<Type> {
        let (owner_key, owner_type) = self.reference(&first_expression(target)?)?;
        let field = tokens(target).filter(|token| token.kind() == SyntaxKind::Identifier).last()?;
        let Type::Object(owner) = self.expand(&owner_type.without_nil()) else {
            return None;
        };
        if !owner.open {
            return None;
        }
        let retyped = retype(self, &owner.properties.get(field.text())?.value, assigned)?;
        let path: Vec<String> = owner_key.path.iter().cloned().chain([field.text().to_string()]).collect();
        let root = owner_key.symbol;
        if let Some(declared) = with_field_type(self.scopes.declared_type(root), &path, &retyped) {
            self.scopes.redeclare(root, declared);
        }
        for depth in 0..path.len() {
            let key = NarrowingKey { symbol: root, path: path[..depth].to_vec() };
            if let Some(narrowed) =
                self.scopes.narrowed(&key).and_then(|narrowed| with_field_type(narrowed, &path[depth..], &retyped))
            {
                self.scopes.narrow(key, narrowed);
            }
        }
        Some(retyped)
    }
}

/// `value` with the field at `path` given type `field_type`, through
/// inline tables and unions of them; `None` when no table on the path
/// has the field.
fn with_field_type(value: &Type, path: &[String], field_type: &Type) -> Option<Type> {
    let Some((name, rest)) = path.split_first() else {
        return Some(field_type.clone());
    };
    match value {
        Type::Object(object) => {
            let property = object.properties.get(name)?;
            let replaced = with_field_type(&property.value, rest, field_type)?;
            let mut rebuilt = ObjectType::clone(object);
            rebuilt.properties.get_mut(name)?.value = replaced;
            Some(Type::Object(Rc::new(rebuilt)))
        }
        Type::Union(members) => {
            let rebuilt: Vec<Option<Type>> =
                members.iter().map(|member| with_field_type(member, path, field_type)).collect();
            if rebuilt.iter().all(Option::is_none) {
                return None;
            }
            let members =
                members.iter().zip(rebuilt).map(|(member, rebuilt)| rebuilt.unwrap_or_else(|| member.clone()));
            Some(Type::union(members))
        }
        _ => None,
    }
}
