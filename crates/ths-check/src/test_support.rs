//! Helpers shared by the checker's topic test files.

use ths_syntax::Dialect;

use crate::{CheckOptions, Declarations, Severity, check, diagnose};

pub(crate) fn errors(source: &str) -> Vec<String> {
    check(source, Dialect::Typed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

/// Nil problems are warnings, so they do not fail a check.
pub(crate) fn warnings(source: &str) -> Vec<String> {
    check(source, Dialect::Typed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

pub(crate) fn assert_clean(source: &str) {
    assert_eq!(errors(source), Vec::<String>::new(), "{source}");
}

pub(crate) fn errors_with_built_ins(source: &str, dialect: Dialect) -> Vec<String> {
    diagnose(source, dialect, &Declarations::built_in(), CheckOptions::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

pub(crate) fn warnings_with_built_ins(source: &str, dialect: Dialect) -> Vec<String> {
    diagnose(source, dialect, &Declarations::built_in(), CheckOptions::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

pub(crate) fn unknown_name(name: &str) -> String {
    format!("Cannot find name '{name}': the server refuses to compile a read of a name that nothing defines.")
}
