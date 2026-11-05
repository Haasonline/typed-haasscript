//! File extensions of Typed HaasScript and what stripping produces.

/// Typed source.
pub const TYPED_SOURCE: &str = ".ths";
/// Declarations only; never stripped or uploaded.
pub const DECLARATION: &str = ".d.ths";
/// Stripped output, uploaded to the TradeServer. Without the dot, for
/// `Path::with_extension`.
pub const UPLOAD: &str = "hs";
/// Plain HaasScript as exported or written by hand.
pub const PLAIN_LUA: &str = ".lua";
pub const PLAIN_UPLOAD: &str = ".hs";
