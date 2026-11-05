//! Multiple return values: checking them by position and spreading a final call over value lists.

use crate::test_support::{assert_clean, errors, warnings};

#[test]
fn return_values_are_checked_by_position() {
    assert_eq!(
        errors("local function f(): (number, string) return 1, 2 end"),
        ["Type '2' is not assignable to type 'string'."]
    );
}

#[test]
fn missing_return_value_is_nil() {
    assert_eq!(
        errors("local function f(): (number, string) return 1 end"),
        ["Type 'nil' is not assignable to type 'string'."]
    );
}

#[test]
fn too_many_return_values_are_reported() {
    assert_eq!(errors("local function f(): number return 1, 2 end"), ["Expected 1 return value(s), but got 2."]);
}

#[test]
fn return_type_is_inferred_with_several_values() {
    assert_eq!(
        errors("local function f() return 1, 'a' end\nlocal a, b = f()\nlocal n: number = b"),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn local_list_takes_every_value_of_a_final_call() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal a, b = pair()\nlocal n: number = b"),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn assignment_list_takes_every_value_of_a_final_call() {
    assert_eq!(
        errors(
            "local function pair(): (number, string) return 1, 'a' end\nlocal a: number, b: number = 0, 0\na, b = pair()"
        ),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn local_list_longer_than_a_final_call_is_reported() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal a, b, c = pair()"),
        ["The call returns 2 value(s), but 3 are required here."]
    );
}

#[test]
fn assignment_list_longer_than_a_final_call_is_reported() {
    assert_eq!(
        errors("local function one(): number return 1 end\nlocal a, b = 0, 0\na, b = one()"),
        ["The call returns 1 value(s), but 2 are required here."]
    );
}

#[test]
fn values_needed_from_a_final_call_exclude_earlier_expressions() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal x, a, b, c = 0, pair()"),
        ["The call returns 2 value(s), but 3 are required here."]
    );
}

#[test]
fn call_with_rest_values_may_fill_any_number_of_locals() {
    assert_clean("declare function words(): ...string\nlocal a, b, c = words()");
}

#[test]
fn unchecked_call_may_fill_any_number_of_locals() {
    assert_clean("local a, b, c = SomeBuiltIn()");
}

#[test]
fn call_inside_an_expression_gives_its_first_value() {
    assert_eq!(
        errors("local function pair(): (number, string) return 1, 'a' end\nlocal s: string = (pair())"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn final_call_spreads_over_the_remaining_parameters() {
    assert_eq!(
        errors(
            "local function pair(): (number, string) return 1, 'a' end\nlocal function g(a: number, b: number) end\ng(pair())"
        ),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn final_call_with_too_few_values_is_reported() {
    assert_eq!(
        errors("local function one(): number return 1 end\nlocal function g(a: number, b: number) end\ng(one())"),
        ["Expected 2 arguments, but got 1."]
    );
}

#[test]
fn unchecked_call_supplies_any_number_of_values() {
    assert_clean("local a: number, b: string = SomeBuiltIn()");
}

#[test]
fn function_returning_several_values_fits_one_expected_value() {
    assert_clean("local f: () => number = function(): (number, string) return 1, 'a' end");
}

#[test]
fn type_list_outside_a_return_type_is_reported() {
    assert_eq!(errors("local x: (number, string) = 1"), ["A list of types is only allowed as a return type."]);
}

#[test]
fn rest_return_values_accept_any_number_of_values() {
    assert_clean("local function f(): (number, ...string) if Random() then return 1 end return 1, 'a', 'b' end");
}

#[test]
fn rest_return_values_are_checked() {
    assert_eq!(
        errors("local function f(): ...string return 'a', 2 end"),
        ["Type '2' is not assignable to type 'string'."]
    );
}

#[test]
fn rest_value_received_alone_may_be_nil() {
    assert_eq!(
        warnings("declare function words(): ...string\nlocal s: string = words()"),
        ["Type 'string | nil' is not assignable to type 'string': the value may be nil."]
    );
}

#[test]
fn rest_values_pass_on_into_rest_parameters() {
    assert_clean("declare function words(): ...string\nlocal function join(...: string) end\njoin(words())");
}

#[test]
fn rest_values_pass_on_into_rest_return_values() {
    assert_clean("declare function words(): ...string\nlocal function more(): ...string return 'a', words() end");
}

#[test]
fn rest_values_never_count_as_too_few_arguments() {
    assert_clean(
        "declare function words(): ...string\nlocal function two(a: string | nil, b: string | nil) end\ntwo(words())",
    );
}

#[test]
fn variadic_type_outside_the_end_of_a_return_type_is_reported() {
    assert_eq!(
        errors("local function f(): (...string, number) return 'a', 1 end"),
        ["A variadic type is only allowed at the end of a return type."]
    );
}
