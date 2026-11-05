//! The file a document's URI names. Only `file:` documents are checked:
//! an unsaved, untitled buffer has no place for its imports to be relative
//! to.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;
use url::Url;

pub fn file_path(uri: &Uri) -> Option<PathBuf> {
    Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

/// The `file:` URI of an absolute path, for pointing the editor at a file
/// it may not have open.
pub fn file_uri(path: &Path) -> Option<Uri> {
    Uri::from_str(Url::from_file_path(path).ok()?.as_str()).ok()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_file_uri_names_its_path_with_escapes_decoded() {
        let uri = Uri::from_str("file:///project/my%20bots/trend.ths").unwrap();
        assert_eq!(file_path(&uri), Some(PathBuf::from("/project/my bots/trend.ths")));
    }

    #[test]
    fn an_untitled_document_names_no_file() {
        let uri = Uri::from_str("untitled:Untitled-1").unwrap();
        assert_eq!(file_path(&uri), None);
    }
}
