//! Which dialect a source file is written in, told by its extension.

use std::path::Path;

use ths_project::file_extensions;
use ths_syntax::Dialect;

/// Typed HaasScript for `.ths` (and `.d.ths`); plain for `.lua` and `.hs`.
pub fn dialect_of(file: &Path) -> Option<Dialect> {
    let name = file.to_string_lossy();
    if name.ends_with(file_extensions::TYPED_SOURCE) {
        Some(Dialect::Typed)
    } else if name.ends_with(file_extensions::PLAIN_LUA) || name.ends_with(file_extensions::PLAIN_UPLOAD) {
        Some(Dialect::Plain)
    } else {
        None
    }
}
