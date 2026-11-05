//! `ths check`: prints every diagnostic as `path:line:column: severity:
//! message`, and fails if any is an error.

use std::path::{Path, PathBuf};

use ths_check::{Diagnostic, Severity};
use ths_syntax::line_column;

use crate::file_check::{Context, diagnostics};
use crate::shown_paths::shown;
use crate::source_dialect::dialect_of;

/// Prints the diagnostics of every one of `files`; returns whether any
/// is an error.
pub fn check_files(files: &[PathBuf], context: &Context) -> bool {
    let mut has_errors = false;
    for file in files {
        match check_file(file, context) {
            Ok(file_has_errors) => has_errors |= file_has_errors,
            Err(message) => {
                eprintln!("{message}");
                has_errors = true;
            }
        }
    }
    has_errors
}

/// Prints the diagnostics of `file`; returns whether it has errors.
fn check_file(file: &Path, context: &Context) -> Result<bool, String> {
    let shown = shown(file);
    let display = shown.display();
    let (source, diagnostics) = checked_file(file, context).map_err(|problem| format!("{display}: {problem}"))?;
    for diagnostic in &diagnostics {
        println!("{display}:{}", diagnostic_line(&source, diagnostic));
    }
    Ok(diagnostics.iter().any(|diagnostic| diagnostic.severity == Severity::Error))
}

/// The text of `file` and its diagnostics, or why it cannot be checked.
fn checked_file(file: &Path, context: &Context) -> Result<(String, Vec<Diagnostic>), String> {
    let dialect = dialect_of(file).ok_or_else(|| "not a .ths, .lua or .hs file".to_string())?;
    let source = std::fs::read_to_string(file).map_err(|error| format!("cannot read: {error}"))?;
    let diagnostics = diagnostics(file, &source, dialect, context);
    Ok((source, diagnostics))
}

/// `line:column: severity: message` for `diagnostic` in `source`.
fn diagnostic_line(source: &str, diagnostic: &Diagnostic) -> String {
    let position = line_column(source, diagnostic.range.start());
    let severity = severity_label(diagnostic.severity);
    format!("{}:{}: {severity}: {}", position.line, position.column, diagnostic.message)
}

/// The word a printed diagnostic names its severity with.
fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}
