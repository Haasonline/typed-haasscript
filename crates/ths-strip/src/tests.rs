use ths_syntax::{Dialect, parse};

use crate::strip;

fn stripped(source: &str) -> String {
    strip(source).unwrap_or_else(|errors| panic!("{source}\n{errors:?}"))
}

#[test]
fn plain_code_is_unchanged() {
    let source = "local x = 1\nif x then Log('a') end\n";
    assert_eq!(stripped(source), source);
}

#[test]
fn output_keeps_every_line_and_column() {
    let source = "local x: number = 1\r\ninterface A {\n  b: string\n}\nlocal y = x as number\n";
    let output = stripped(source);
    assert_eq!(output.lines().count(), source.lines().count());
    for (output_line, source_line) in output.lines().zip(source.lines()) {
        assert_eq!(output_line.chars().count(), source_line.chars().count());
    }
}

#[test]
fn variable_annotation_is_blanked() {
    assert_eq!(stripped("local x: number = 1"), "local x         = 1");
}

#[test]
fn parameter_annotations_optional_marker_and_return_type_are_blanked() {
    assert_eq!(stripped("function f(a: number, b?: string): T end"), "function f(a        , b         )    end");
}

#[test]
fn return_type_list_is_blanked() {
    assert_eq!(stripped("function f(): (number, string) end"), "function f()                   end");
}

#[test]
fn rest_return_values_are_blanked() {
    assert_eq!(stripped("function f(): (number, ...string) end"), "function f()                      end");
}

#[test]
fn declare_type_is_blanked() {
    assert_eq!(stripped("declare type A extends B\nx = 1"), "                        \nx = 1");
}

#[test]
fn declare_type_with_a_call_signature_is_blanked() {
    assert_eq!(stripped("declare type A { (b: number): A }\nx = 1"), "                                 \nx = 1");
}

#[test]
fn list_cast_is_blanked() {
    assert_eq!(stripped("local a, b = f() as (number, string)"), "local a, b = f()                    ");
}

#[test]
fn explicit_type_arguments_are_blanked() {
    assert_eq!(
        stripped("local y = f<<number, string>>(x) + t:get<<number>>('k')"),
        "local y = f                  (x) + t:get          ('k')"
    );
}

#[test]
fn type_parameters_are_blanked() {
    assert_eq!(stripped("function f<T>(a: T) end"), "function f   (a   ) end");
}

#[test]
fn type_alias_is_blanked_but_its_newlines_stay() {
    assert_eq!(stripped("type A =\n  number\nx = 1"), "        \n        \nx = 1");
}

#[test]
fn cast_tail_is_blanked_and_the_expression_stays() {
    assert_eq!(stripped("x = f() as number"), "x = f()          ");
}

#[test]
fn nested_casts_are_blanked() {
    assert_eq!(stripped("x = a as T as U"), "x = a          ");
}

#[test]
fn annotation_inside_a_cast_type_is_blanked_once() {
    assert_eq!(stripped("x = f as (a: number) => T"), "x = f                    ");
}

#[test]
fn removed_declaration_before_a_parenthesis_becomes_a_separator() {
    assert_eq!(stripped("local x = y\ntype A = number\n(z)()"), "local x = y\n;              \n(z)()");
}

#[test]
fn removed_cast_before_a_parenthesis_becomes_a_separator() {
    assert_eq!(stripped("local x = y as T\n(z)()"), "local x = y ;   \n(z)()");
}

#[test]
fn only_the_last_of_several_declarations_gets_the_separator() {
    assert_eq!(stripped("x = y\ntype A = B\ntype C = D\n(z)()"), "x = y\n          \n;         \n(z)()");
}

#[test]
fn no_separator_when_the_next_statement_does_not_start_with_a_parenthesis() {
    assert_eq!(stripped("type A = B\nx = 1"), "          \nx = 1");
}

#[test]
fn annotation_before_a_parenthesis_needs_no_separator() {
    // `local x` cannot be continued by `(`, so Lua already ends it there.
    assert_eq!(stripped("local x: T\n(z)()"), "local x   \n(z)()");
}

#[test]
fn syntax_errors_are_returned_instead_of_output() {
    assert!(strip("local x: = 1").is_err());
}

#[test]
fn output_parses_as_plain_haasscript() {
    let source = "\
interface Signal { direction: string; strength?: number }
declare function ClosePrices(interval?: number): number[]
local function average<T>(values: T[], count: number): number
  local total: number = 0
  for i: number = 1, count do total = total + (values[i] as number) end
  return total / count
end
local current = Load('signal') as Signal
(Log)(average({1, 2}, 2))
";
    let output = stripped(source);
    let reparsed = parse(&output, Dialect::Plain);
    assert!(reparsed.errors.is_empty(), "{output}\n{:?}", reparsed.errors);
}

#[test]
fn export_is_blanked_and_the_declaration_stays() {
    assert_eq!(stripped("export local limit = 10"), "       local limit = 10");
}

#[test]
fn an_exported_type_is_blanked_whole() {
    assert_eq!(stripped("export type Prices = number[]"), " ".repeat(29));
}

#[test]
fn imports_and_export_lists_are_left_to_the_bundler() {
    assert_eq!(stripped("import { a } from \"./a\""), " ".repeat(23));
    assert_eq!(stripped("local b = 1\nexport { b }"), "local b = 1\n            ");
}

// Compact stripping, for bundles

fn compact(source: &str) -> String {
    crate::strip_compact(source).unwrap_or_else(|errors| panic!("{source}\n{errors:?}"))
}

#[test]
fn compact_stripping_cuts_annotations_out() {
    assert_eq!(
        compact("local function round(value: number, places?: number): number\n    return value\nend"),
        "local function round(value, places)\n    return value\nend"
    );
}

#[test]
fn compact_stripping_drops_the_line_of_a_removed_statement() {
    assert_eq!(
        compact("import { type Quote } from \"./quotes\"\ninterface Pair {\n  a: number\n}\nlocal x = 1\n"),
        "local x = 1\n"
    );
}

#[test]
fn compact_stripping_removes_export_and_its_space() {
    assert_eq!(compact("export local limit = 1\nexport { limit as maximum }\n"), "local limit = 1\n");
}

#[test]
fn compact_stripping_keeps_names_apart() {
    assert_eq!(compact("local a = b as number local c = 1"), "local a = b  local c = 1");
}

#[test]
fn compact_stripping_keeps_statements_apart_before_a_parenthesis() {
    assert_eq!(compact("local a = b as T\n(f)()"), "local a = b ;\n(f)()");
}

#[test]
fn compact_stripping_drops_an_exported_type_line_and_keeps_blank_lines() {
    assert_eq!(
        compact("local a = 1\n\nexport interface Quote { bid: number }\n    export type Side = \"buy\"\nlocal b = 2\n"),
        "local a = 1\n\nlocal b = 2\n"
    );
}
