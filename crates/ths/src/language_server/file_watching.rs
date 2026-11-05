//! Asks the editor to tell the server when files that open documents can
//! see change on disk: a module, a custom command, a declaration file or a
//! `ths.toml` changed outside the editor, by a `git pull` for instance.
//! Only for an editor that takes the request; others send the events
//! unasked or not at all.

use lsp_server::{Request, RequestId};
use lsp_types::notification::{DidChangeWatchedFiles, Notification as _};
use lsp_types::request::{RegisterCapability, Request as _};
use lsp_types::{
    ClientCapabilities, DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, Registration,
    RegistrationParams,
};
use ths_project::{SETTINGS_FILE, file_extensions};

const REGISTRATION_ID: &str = "ths-watched-files";
const REQUEST_ID: &str = "ths-register-watched-files";
/// Every script extension a check reads: typed, uploaded and plain.
const WATCHED_EXTENSIONS: [&str; 3] =
    [file_extensions::TYPED_SOURCE, file_extensions::PLAIN_UPLOAD, file_extensions::PLAIN_LUA];
const EXTENSION_DOT: char = '.';

/// Whether the editor takes a request to watch files.
pub fn editor_watches_files(capabilities: &ClientCapabilities) -> bool {
    capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files.as_ref())
        .and_then(|watching| watching.dynamic_registration)
        .unwrap_or(false)
}

/// Glob patterns for the scripts and settings files whose changes can
/// change a check.
fn watched_patterns() -> [String; 2] {
    let extensions = WATCHED_EXTENSIONS.map(|extension| extension.trim_start_matches(EXTENSION_DOT));
    [format!("**/*.{{{}}}", extensions.join(",")), format!("**/{SETTINGS_FILE}")]
}

pub fn registration_request() -> Request {
    let watchers = watched_patterns()
        .into_iter()
        .map(|pattern| FileSystemWatcher { glob_pattern: GlobPattern::String(pattern), kind: None })
        .collect();
    let options = DidChangeWatchedFilesRegistrationOptions { watchers };
    let registration = Registration {
        id: REGISTRATION_ID.to_string(),
        method: DidChangeWatchedFiles::METHOD.to_string(),
        register_options: serde_json::to_value(options).ok(),
    };
    let parameters = RegistrationParams { registrations: vec![registration] };
    Request::new(RequestId::from(REQUEST_ID.to_string()), RegisterCapability::METHOD.to_string(), parameters)
}

#[cfg(test)]
mod tests {
    use super::watched_patterns;

    #[test]
    fn the_editor_watches_every_script_and_settings_file() {
        assert_eq!(watched_patterns(), ["**/*.{ths,hs,lua}".to_string(), "**/ths.toml".to_string()]);
    }
}
