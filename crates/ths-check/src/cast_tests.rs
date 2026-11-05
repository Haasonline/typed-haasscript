//! Casts: allowed and unrelated casts, casts on calls that spread several values, and list casts.

use crate::test_support::{assert_clean, errors, warnings};

#[test]
fn cast_on_a_call_with_rest_values_is_reported() {
    assert_eq!(
        errors("declare function words(): ...string\ng(1, words() as string)"),
        ["Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first."]
    );
}

#[test]
fn cast_from_any_is_allowed() {
    assert_clean("local x = Load('k') as number\nlocal y = x + 1");
}

#[test]
fn unrelated_cast_is_reported() {
    assert_eq!(errors("local x = 'a' as number"), ["Conversion of type '\"a\"' to type 'number' may be a mistake."]);
}

#[test]
fn cast_on_a_call_ending_an_argument_list_is_reported() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\ng(1, pair() as number)"),
        ["Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first."]
    );
}

#[test]
fn cast_on_a_call_ending_a_return_is_reported() {
    assert_eq!(
        errors(
            "local function pair(): (number, string) return 1, 'a' end\nlocal function f() return pair() as number end"
        ),
        ["Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first."]
    );
}

#[test]
fn cast_on_a_call_ending_a_table_constructor_is_reported() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal t = { 1, pair() as number }"),
        ["Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first."]
    );
}

#[test]
fn cast_on_a_call_spreading_over_more_locals_is_reported() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal a, b = pair() as number"),
        ["Once stripped, this cast keeps every value the call returns; write '(call) as Type' to keep only the first."]
    );
}

#[test]
fn cast_on_a_call_with_one_target_is_allowed() {
    assert_clean("local function pair(): (number, string) return 1, 'a' end\nlocal a = pair() as number");
}

#[test]
fn cast_on_a_parenthesized_call_is_allowed() {
    assert_clean("local function pair(): (number, string) return 1, 'a' end\ng(1, (pair()) as number)");
}

#[test]
fn cast_on_a_call_returning_one_value_is_allowed() {
    assert_clean("local function one(): number return 1 end\ng(1, one() as number)");
}

#[test]
fn list_cast_types_every_value() {
    assert_eq!(
        errors("local a, b = SomeBuiltIn() as (number, string)\nlocal n: number = b"),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn list_cast_spreads_into_arguments_without_the_truncation_error() {
    assert_eq!(
        errors("local function g(a: number, b: number) end\ng(SomeBuiltIn() as (number, string))"),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn list_cast_inside_an_expression_gives_its_first_value() {
    assert_eq!(
        errors("local s: string = (SomeBuiltIn() as (number, string))"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn list_cast_with_rest_values_may_give_nil() {
    assert_eq!(
        warnings("local function f(...: any) local a, b = ... as (number, ...string) local s: string = b end"),
        ["Type 'string | nil' is not assignable to type 'string': the value may be nil."]
    );
}

#[test]
fn list_cast_of_something_other_than_a_call_is_reported() {
    assert_eq!(
        errors("local x = 1\nlocal a, b = x as (number, string)"),
        ["Only a call or '...' can be cast to a list of types."]
    );
}

#[test]
fn unrelated_list_cast_is_reported() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal a, b = pair() as (string, string)"),
        ["Conversion of type '(number, string)' to type '(string, string)' may be a mistake."]
    );
}
