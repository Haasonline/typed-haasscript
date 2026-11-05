//! Dialects and file-wide rules: strict mode, what a plain file reports, syntax errors and non-ASCII text.

use ths_syntax::Dialect;

use crate::test_support::{assert_clean, errors, unknown_name};
use crate::{CheckOptions, Declarations, Severity, check, check_with, diagnose};

// Text

#[test]
fn non_ascii_text_is_a_warning() {
    let diagnostics = check("Log('\u{e9}')", Dialect::Typed);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, Severity::Warning);
}

#[test]
fn syntax_errors_are_included() {
    assert_eq!(errors("local x: = 1"), ["expected a type"]);
}

// Strict

fn strict_errors(source: &str, dialect: Dialect) -> Vec<String> {
    diagnose(source, dialect, &Declarations::default(), CheckOptions { strict: true, ..CheckOptions::default() })
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn strict_reports_an_unannotated_parameter() {
    assert_eq!(
        strict_errors("local function f(x) return x end", Dialect::Typed),
        ["Parameter 'x' implicitly has an 'any' type."]
    );
}

#[test]
fn strict_accepts_an_annotated_parameter() {
    assert_eq!(strict_errors("local function f(x: number) return x end", Dialect::Typed), Vec::<String>::new());
}

#[test]
fn strict_accepts_a_parameter_typed_by_its_context() {
    assert_eq!(
        strict_errors("local function each(f: (n: number) => nil) end\neach(function(n) end)", Dialect::Typed),
        Vec::<String>::new()
    );
}

#[test]
fn strict_reports_an_unannotated_lambda_parameter() {
    assert_eq!(strict_errors("local f = |x| x", Dialect::Typed), ["Parameter 'x' implicitly has an 'any' type."]);
}

#[test]
fn strict_leaves_an_unannotated_rest_parameter_alone() {
    assert_eq!(strict_errors("local function f(...) end", Dialect::Typed), Vec::<String>::new());
}

#[test]
fn strict_reports_a_local_with_neither_annotation_nor_value() {
    assert_eq!(strict_errors("local x\nx = 1", Dialect::Typed), ["Variable 'x' implicitly has an 'any' type."]);
}

#[test]
fn strict_accepts_a_local_inferred_from_its_value() {
    assert_eq!(strict_errors("local x = 1", Dialect::Typed), Vec::<String>::new());
}

#[test]
fn strict_does_not_apply_to_plain_scripts() {
    assert_eq!(strict_errors("local function f(x) local y end", Dialect::Plain), Vec::<String>::new());
}

#[test]
fn a_plain_file_reports_no_type_errors() {
    let source = "local average = SMA(ClosePrices(), 'ten')\nlocal x = 1\nlocal y = x.price";
    assert_eq!(check_with(source, Dialect::Plain, &Declarations::built_in(), CheckOptions::default()), []);
}

#[test]
fn a_plain_file_reports_unknown_names() {
    let messages: Vec<String> =
        check_with("local value = Missing", Dialect::Plain, &Declarations::built_in(), CheckOptions::default())
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect();
    assert_eq!(messages, [unknown_name("Missing")]);
}

#[test]
fn a_plain_file_reports_its_syntax_errors() {
    assert_eq!(check("local = 1", Dialect::Plain).len(), check("local = 1", Dialect::Typed).len());
    assert!(!check("local = 1", Dialect::Plain).is_empty());
}

#[test]
fn implicit_any_is_not_reported_without_strict() {
    assert_clean("local function f(x) local y end");
}
