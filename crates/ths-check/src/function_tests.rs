//! Functions: inferred and annotated returns, argument checks, optional and rest parameters, and callbacks.

use crate::test_support::{assert_clean, errors, warnings};

#[test]
fn return_type_is_inferred() {
    assert_eq!(
        errors("local function f() return 1 end\nlocal x: string = f()"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn return_value_is_checked_against_the_annotation() {
    assert_eq!(
        errors("local function f(): number return 'a' end"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn missing_return_is_reported_when_nil_is_not_allowed() {
    assert_eq!(
        errors("local function f(x: boolean): number\n  if x then return 1 end\nend"),
        ["Function lacks ending return statement and return type 'number' does not include 'nil'."]
    );
}

#[test]
fn returns_in_every_if_branch_count_as_an_ending_return() {
    assert_clean("local function f(x: boolean): number\n  if x then return 1 else return 2 end\nend");
}

#[test]
fn argument_types_are_checked() {
    assert_eq!(errors("local function f(a: number) end\nf('a')"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn too_few_arguments_are_reported() {
    assert_eq!(errors("local function f(a: number, b: number) end\nf(1)"), ["Expected 2 arguments, but got 1."]);
}

#[test]
fn optional_parameters_may_be_left_out() {
    assert_clean("local function f(a: number, b?: number) end\nf(1)");
}

#[test]
fn optional_parameter_may_be_nil_inside_the_function() {
    assert_eq!(warnings("local function f(b?: number) return b + 1 end"), ["Object is possibly 'nil'."]);
}

#[test]
fn too_many_arguments_are_reported() {
    assert_eq!(errors("local function f(a: number) end\nf(1, 2)"), ["Expected 1 arguments, but got 2."]);
}

#[test]
fn rest_parameter_accepts_extra_arguments() {
    assert_clean("local function f(...: number) end\nf(1, 2, 3)");
}

#[test]
fn calling_a_number_is_reported() {
    assert_eq!(errors("local x = 1\nx()"), ["This expression is not callable. Type 'number' has no call signatures."]);
}

#[test]
fn declared_function_is_visible_before_its_declaration() {
    assert_eq!(
        errors("local x: string = ClosePrices()\ndeclare function ClosePrices(): number[]"),
        ["Type 'number[]' is not assignable to type 'string'."]
    );
}

#[test]
fn callback_parameters_take_the_expected_type() {
    assert_eq!(
        errors("local function each(f: (value: number) => nil) end\neach(function(value) local s: string = value end)"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn typed_lambda_is_inferred() {
    assert_eq!(
        errors("local double = |x: number| x * 2\nlocal s: string = double(1)"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn a_function_statement_not_named_yet_reports_only_the_missing_name() {
    assert_eq!(errors("function () end"), ["expected a function name"]);
}

#[test]
fn the_body_of_a_function_statement_not_named_yet_is_still_checked() {
    assert_eq!(
        errors("function ()\n  local x: number = 'a'\nend"),
        ["expected a function name", "Type '\"a\"' is not assignable to type 'number'."]
    );
}
