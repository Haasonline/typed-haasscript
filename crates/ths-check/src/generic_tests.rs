//! Generics: inferred and explicit type arguments, constraints, and generic interfaces.

use crate::test_support::{assert_clean, errors};

#[test]
fn generic_call_returns_the_inferred_type_argument() {
    assert_eq!(
        errors("local function identity<T>(value: T): T return value end\nlocal s: string = identity(1)"),
        ["Type '1' is not assignable to type 'string'."]
    );
}

#[test]
fn inferred_type_argument_must_satisfy_the_constraint() {
    assert_eq!(
        errors("local function twice<T extends number>(value: T): T return value end\ntwice('a')"),
        ["Type '\"a\"' does not satisfy the constraint 'number'."]
    );
}

#[test]
fn type_parameter_is_opaque_inside_the_function() {
    assert_eq!(
        errors("local function f<T>(value: T): number return value end"),
        ["Type 'T' is not assignable to type 'number'."]
    );
}

#[test]
fn constrained_type_parameter_is_usable_as_its_constraint() {
    assert_clean("local function double<T extends number>(value: T): number return value * 2 end");
}

#[test]
fn callback_parameter_is_typed_from_an_earlier_argument() {
    assert_eq!(
        errors(
            "local function map<T, U>(items: T[], f: (item: T) => U): U[] return {} end\nlocal xs: number[] = { 1, 2 }\nlocal names: string[] = map(xs, |x| x * 2)"
        ),
        ["Type 'number[]' is not assignable to type 'string[]'."]
    );
}

#[test]
fn generic_interface_substitutes_its_type_argument() {
    assert_eq!(
        errors("interface Box<T> { value: T }\nlocal b: Box<number> = { value = 'a' }"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn missing_type_argument_is_reported() {
    assert_eq!(
        errors("interface Box<T> { value: T }\nlocal b: Box = { value = 1 }"),
        ["Generic type 'Box' requires 1 type argument(s)."]
    );
}

#[test]
fn defaulted_type_argument_may_be_left_out() {
    assert_eq!(
        errors("interface Box<T = number> { value: T }\nlocal b: Box = { value = 'a' }"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn recursive_generic_interface_is_checked_without_looping() {
    assert_clean(
        "interface List<T> { value: T; next: List<T> | nil }\nlocal tail: List<number> = { value = 2, next = nil }\nlocal head: List<number> = { value = 1, next = tail }",
    );
}

#[test]
fn recursive_generic_interface_with_other_arguments_is_reported_without_looping() {
    assert_eq!(
        errors(
            "interface List<T> { value: T; next: List<T> | nil }\nlocal tail: List<string> = { value = 'a', next = nil }\nlocal head: List<number> = { value = 1, next = tail }"
        ),
        ["Type 'List<string>' is not assignable to type 'List<number> | nil'."]
    );
}

#[test]
fn recursive_generic_interface_compared_through_a_union_is_checked_without_looping() {
    assert_clean(
        "interface Chain<T> { next: Chain<T> | nil }\nlocal tail: Chain<string> = { next = nil }\nlocal head: Chain<number> | nil = tail",
    );
}

#[test]
fn generic_interface_growing_with_each_expansion_is_checked_without_looping() {
    assert_eq!(
        errors(
            "interface Nest<T> { inner: Nest<Nest<T>> | nil; value: T }\nlocal a: Nest<string> = { inner = nil, value = 'a' }\nlocal b: Nest<number> = a"
        ),
        ["Type 'Nest<string>' is not assignable to type 'Nest<number>'."]
    );
}

#[test]
fn explicit_type_argument_checks_the_arguments() {
    assert_eq!(
        errors("local function identity<T>(value: T): T return value end\nidentity<<number>>('a')"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn explicit_type_argument_sets_the_return_type() {
    assert_eq!(
        errors("declare function make<T>(): T\nlocal s: string = make<<number>>()"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn explicit_type_arguments_bind_the_leading_parameters_only() {
    assert_eq!(
        errors("declare function second<A, B>(a: A, b: B): B\nlocal n: number = second<<number>>(1, 'a')"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn too_many_explicit_type_arguments_are_reported() {
    assert_eq!(
        errors("local function identity<T>(value: T): T return value end\nidentity<<number, string>>(1)"),
        ["Expected 1 type argument(s), but got 2."]
    );
}

#[test]
fn explicit_type_argument_must_satisfy_the_constraint() {
    assert_eq!(
        errors("declare function twice<T extends number>(value: T): T\ntwice<<string>>('a')"),
        ["Type 'string' does not satisfy the constraint 'number'."]
    );
}

#[test]
fn explicit_type_arguments_on_a_non_generic_function_are_reported() {
    assert_eq!(
        errors("local function f(x: number) end\nf<<number>>(1)"),
        ["Type '(x: number) => nil' takes no type arguments."]
    );
}

#[test]
fn explicit_type_arguments_apply_to_a_method_call() {
    assert_eq!(
        errors(
            "interface Store { get<T>(key: string): T }\nlocal function run(s: Store) local n: string = s:get<<number>>('k') end"
        ),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn instantiation_without_a_call_is_a_function_value() {
    assert_eq!(
        errors("local function identity<T>(value: T): T return value end\nlocal f = identity<<number>>\nf('a')"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}
