//! The category of an enumeration constant: the type extending `Enum`
//! that a value such as `SmaType` has (`EnumMovingAverages`). Constants
//! of two categories cannot be compared (verified on the TradeServer: the
//! script stops with "Invalid constant compare … Commands are from
//! different categories.").

use crate::engine_types::ENUM_TYPE;
use crate::type_environment::TypeEnvironment;
use crate::types::Type;

/// The one category `value` belongs to, if it is a constant of a named
/// category rather than any `Enum`.
pub(crate) fn category_of(environment: &TypeEnvironment, value: &Type) -> Option<Type> {
    let [member] = value.members() else { return None };
    match environment.expand(member) {
        Type::Opaque(name) if name != ENUM_TYPE && environment.opaque_extends(&name, ENUM_TYPE) => {
            Some(Type::Opaque(name))
        }
        _ => None,
    }
}
