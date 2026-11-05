use std::fmt;
use std::path::PathBuf;

/// Why a project's settings or files could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectError {
    Read { path: PathBuf, message: String },
    Settings { path: PathBuf, message: String },
    Pattern { pattern: String, message: String },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Read { path, message } => write!(formatter, "{}: cannot read: {message}", path.display()),
            ProjectError::Settings { path, message } => write!(formatter, "{}: {message}", path.display()),
            ProjectError::Pattern { pattern, message } => write!(formatter, "pattern '{pattern}': {message}"),
        }
    }
}
