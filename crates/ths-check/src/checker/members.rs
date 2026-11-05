//! Field access.

use rowan::TextRange;

use super::Checker;
use crate::types::Type;

impl Checker<'_> {
    /// The type of `owner.name`. `owner` must already be nil-free.
    pub(super) fn member_type(&mut self, owner: &Type, name: &str, range: TextRange) -> Type {
        let expanded = self.expand(owner);
        if let Type::Union(members) = &expanded {
            let found: Vec<Type> = members.iter().map(|member| self.member_type(member, name, range)).collect();
            return Type::union(found);
        }
        match expanded {
            Type::Object(object) => {
                if let Some(property) = object.properties.get(name) {
                    return if property.optional {
                        Type::union([property.value.clone(), Type::Nil])
                    } else {
                        property.value.clone()
                    };
                }
                if let Some((key, value)) = &object.index
                    && self.is_assignable(&Type::String, key)
                {
                    return value.clone();
                }
                if object.open {
                    return Type::Any;
                }
                self.sink.error(range, format!("Property '{name}' does not exist on type '{owner}'."));
                Type::Any
            }
            // String methods (`text:upper()`) and engine tables are not
            // declared yet.
            Type::Any | Type::Unknown | Type::String | Type::StringLiteral(_) | Type::Array(_) => Type::Any,
            Type::Never => Type::Never,
            _ => {
                self.sink.error(range, format!("Property '{name}' does not exist on type '{owner}'."));
                Type::Any
            }
        }
    }
}
