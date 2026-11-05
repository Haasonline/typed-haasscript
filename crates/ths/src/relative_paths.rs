//! Paths written from another directory than the current one.

use std::path::{Path, PathBuf};

use ths_project::normalized;

/// `path` from the directory `base`, stepping up with `..` where needed.
pub fn relative_path(path: &Path, base: &Path) -> PathBuf {
    let (Ok(path), Ok(base)) = (std::path::absolute(path), std::path::absolute(base)) else {
        return path.to_path_buf();
    };
    let path = normalized(&path);
    let base = normalized(&base);
    let shared = path.components().zip(base.components()).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for _ in base.components().skip(shared) {
        relative.push("..");
    }
    for component in path.components().skip(shared) {
        relative.push(component);
    }
    relative
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_relative_to_the_script_directory() {
        let base = Path::new("project/bots");
        assert_eq!(relative_path(Path::new("project/bots/trend.ths"), base), PathBuf::from("trend.ths"));
        assert_eq!(relative_path(Path::new("project/lib/quotes.ths"), base), PathBuf::from("../lib/quotes.ths"));
    }
}
