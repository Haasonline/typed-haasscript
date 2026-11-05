//! The names of the engine types the checker gives rules of their own.
//! The built-in declarations declare them, as opaque types
//! (`declare type Name`) or as interfaces; the checker recognizes them by
//! name.

/// The engine's series: price data, indicator results and `HNC()`.
pub const SERIES_TYPE: &str = "HaasNumberCollection";

/// The base every enumeration category extends.
pub const ENUM_TYPE: &str = "Enum";

/// The color category, whose constants do not compare by name.
pub const COLOR_TYPE: &str = "EnumColor";

/// The container `CurrentPrice()` returns, named by the declarations
/// generator after the command.
pub const CURRENT_PRICE_TYPE: &str = "CurrentPriceResult";
