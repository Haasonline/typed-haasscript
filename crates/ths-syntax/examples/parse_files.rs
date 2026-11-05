//! Parses each file given on the command line as plain HaasScript and
//! prints its syntax errors. Exits with 1 if any file has errors.
//!
//! cargo run --example parse_files -- path/to/script.lua ...

use std::process::ExitCode;

use ths_syntax::{Dialect, ParseError, line_column, parse};

fn main() -> ExitCode {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        failed |= !check_file(&path);
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// Reads and parses the file at `path`, printing every problem found.
/// Returns true when the file was read and has no syntax errors.
fn check_file(path: &str) -> bool {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: cannot read: {error}");
            return false;
        }
    };
    let parsed = parse(&source, Dialect::Plain);
    for error in &parsed.errors {
        println!("{}", format_error(path, &source, error));
    }
    parsed.errors.is_empty()
}

/// One report line for `error`: `path:line: message`.
fn format_error(path: &str, source: &str, error: &ParseError) -> String {
    let line = line_column(source, error.range.start()).line;
    format!("{path}:{line}: {}", error.message)
}
