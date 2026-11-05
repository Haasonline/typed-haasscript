//! Interfaces, type aliases and tables: object literals, properties, arrays and methods.

use crate::test_support::{assert_clean, errors, warnings};

#[test]
fn object_literal_with_the_right_fields_fits() {
    assert_clean("interface Point { x: number; y: number }\nlocal p: Point = { x = 1, y = 2 }");
}

#[test]
fn missing_required_property_is_reported() {
    assert_eq!(
        errors("interface Point { x: number; y: number }\nlocal p: Point = { x = 1 }"),
        ["Property 'y' is missing but required in type 'Point'."]
    );
}

#[test]
fn excess_property_is_reported() {
    assert_eq!(
        errors("interface Point { x: number }\nlocal p: Point = { x = 1, z = 2 }"),
        ["Object literal may only specify known properties, and 'z' does not exist in type 'Point'."]
    );
}

#[test]
fn optional_property_reads_as_possibly_nil() {
    assert_eq!(
        warnings("interface Settings { label?: string }\nlocal s: Settings = {}\nlocal n = #s.label"),
        ["Object is possibly 'nil'."]
    );
}

#[test]
fn unknown_property_is_reported() {
    assert_eq!(
        errors("interface Point { x: number }\nlocal p: Point = { x = 1 }\nlocal z = p.z"),
        ["Property 'z' does not exist on type 'Point'."]
    );
}

#[test]
fn interface_extends_a_later_interface() {
    assert_clean("interface B extends A { b: number }\ninterface A { a: number }\nlocal x: B = { a = 1, b = 2 }");
}

#[test]
fn recursive_interface_is_checked_without_looping() {
    assert_clean(
        "interface Node { value: number; next: Node | nil }\nlocal tail: Node = { value = 2, next = nil }\nlocal head: Node = { value = 1, next = tail }",
    );
}

#[test]
fn string_literal_union_rejects_other_strings() {
    assert_eq!(
        errors("type Side = 'long' | 'short'\nlocal s: Side = 'flat'"),
        ["Type '\"flat\"' is not assignable to type 'Side'."]
    );
}

#[test]
fn array_elements_are_checked() {
    assert_eq!(errors("local xs: number[] = { 1, 'a' }"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn empty_table_fits_an_array() {
    assert_clean("local xs: number[] = {}");
}

#[test]
fn inferred_array_has_its_element_type() {
    assert_eq!(
        errors("local xs = { 1, 2 }\nlocal s: string = xs[1]"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn method_call_checks_arguments() {
    assert_eq!(
        errors("interface Bot { step(amount: number): nil }\nlocal function run(bot: Bot) bot:step('a') end"),
        ["Type '\"a\"' is not assignable to type 'number'."]
    );
}

#[test]
fn method_definition_gets_self_of_the_owner_type() {
    assert_eq!(
        errors(
            "interface Bot { size: number; step(): nil }\nlocal bot: Bot = { size = 1, step = function() end }\nfunction bot:step() local s: string = self.size end"
        ),
        ["Type 'number' is not assignable to type 'string'."]
    );
}
