//! What a `ths.toml` holds. Names follow TypeScript's `tsconfig.json`
//! where one exists; an unknown key is an error, as in TypeScript.

use std::path::PathBuf;

use serde::Deserialize;

/// The settings file's name.
pub const SETTINGS_FILE: &str = "ths.toml";

/// Every source file below the project directory, TypeScript's default.
const INCLUDE_EVERYTHING: &str = "**/*";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Files and directories to check and strip, relative to the project
    /// directory, as glob patterns (`tsconfig.json`'s `include`).
    pub include: Vec<String>,
    /// Patterns left out of `include` (`exclude`).
    pub exclude: Vec<String>,
    /// Where stripped files go, keeping their paths below `root`; beside
    /// each source when absent (`outDir`).
    pub output: Option<PathBuf>,
    /// The directory whose layout `output` copies; the common directory of
    /// the stripped sources when absent (`rootDir`).
    pub root: Option<PathBuf>,
    pub check: CheckSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            include: vec![INCLUDE_EVERYTHING.to_string()],
            exclude: Vec::new(),
            output: None,
            root: None,
            check: CheckSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CheckSettings {
    /// Report implicit `any` (`strict`'s `noImplicitAny`).
    pub strict: bool,
    /// Warn where a read falls back past a `local` that ended.
    pub shadowing: bool,
    /// `ths strip` writes nothing when any file has an error
    /// (`noEmitOnError`).
    pub refuse_output_on_errors: bool,
}

impl Settings {
    pub fn parse(text: &str) -> Result<Settings, String> {
        toml::from_str(text).map_err(|error| error.message().to_string())
    }
}
