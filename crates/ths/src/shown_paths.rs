//! How a path is written in a message to the owner.

use std::path::{Path, PathBuf};

/// `path` relative to the current directory when it is below it, for
/// messages.
pub fn shown(path: &Path) -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|current| path.strip_prefix(current).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

/// `path` written with `/` between its parts on every platform, as paths
/// are written in scripts (`import("lib/quotes")`) and bundle labels.
pub fn slash_separated(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::slash_separated;

    #[test]
    fn a_path_is_written_with_slashes_on_every_platform() {
        assert_eq!(slash_separated(&Path::new("lib").join("quotes.ths")), "lib/quotes.ths");
    }
}
