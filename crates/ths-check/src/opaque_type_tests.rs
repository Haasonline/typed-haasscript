//! Opaque declared types: their bases, their lack of fields, and call signatures.

use crate::test_support::{assert_clean, errors};

// Opaque types

#[test]
fn opaque_type_fits_its_base() {
    assert_clean(
        "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\nlocal e: Enum = SignalLong",
    );
}

#[test]
fn opaque_type_does_not_fit_a_sibling() {
    assert_eq!(
        errors(
            "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\nlocal c: EnumColor = SignalLong"
        ),
        ["Type 'EnumSignal' is not assignable to type 'EnumColor'."]
    );
}

#[test]
fn opaque_base_does_not_fit_a_derived_type() {
    assert_eq!(
        errors(
            "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\ndeclare anything: Enum\nlocal s: EnumSignal = anything"
        ),
        ["Type 'Enum' is not assignable to type 'EnumSignal'."]
    );
}

#[test]
fn opaque_type_has_no_fields() {
    assert_eq!(
        errors(
            "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\nlocal x = SignalLong.value"
        ),
        ["Property 'value' does not exist on type 'EnumSignal'."]
    );
}

// Callable opaque types

const COLORS: &str = "declare type Enum\ndeclare type EnumColor extends Enum { (opacity: number): EnumColor }\ndeclare type EnumBrightColor extends EnumColor\ndeclare Red: EnumColor\ndeclare Gold: EnumBrightColor\n";

fn color_errors(body: &str) -> Vec<String> {
    errors(&format!("{COLORS}{body}"))
}

#[test]
fn calling_a_callable_opaque_type_gives_its_return_type() {
    assert_clean(&format!("{COLORS}local faded: EnumColor = Red(50)"));
}

#[test]
fn a_callable_opaque_value_is_still_its_type() {
    assert_clean(&format!("{COLORS}local color: Enum = Red"));
}

#[test]
fn a_callable_opaque_type_checks_its_arguments() {
    assert_eq!(color_errors("local faded = Red('half')"), ["Type '\"half\"' is not assignable to type 'number'."]);
}

#[test]
fn a_callable_opaque_type_requires_its_arguments() {
    assert_eq!(color_errors("local faded = Red()"), ["Expected 1 arguments, but got 0."]);
}

#[test]
fn a_call_signature_is_inherited_from_the_base() {
    assert_clean(&format!("{COLORS}local faded: EnumColor = Gold(50)"));
}

#[test]
fn an_opaque_type_without_a_call_signature_is_not_callable() {
    assert_eq!(
        errors("declare type Enum\ndeclare SignalLong: Enum\nlocal x = SignalLong()"),
        ["This expression is not callable. Type 'Enum' has no call signatures."]
    );
}

#[test]
fn a_declared_type_body_may_only_have_call_signatures() {
    assert_eq!(errors("declare type Enum { name: string }"), ["A declared type may only have call signatures."]);
}

#[test]
fn a_declared_type_may_have_only_one_call_signature() {
    assert_eq!(
        errors("declare type Enum { (a: number): Enum; (a: string): Enum }"),
        ["A declared type may have only one call signature."]
    );
}

#[test]
fn a_call_signature_in_an_interface_is_reported() {
    assert_eq!(
        errors("interface Factory { (value: number): number }"),
        ["Call signatures are only supported in 'declare type'."]
    );
}

#[test]
fn the_length_of_an_opaque_type_other_than_a_series_is_reported() {
    assert_eq!(
        errors("declare type Enum\ndeclare SignalLong: Enum\nlocal n = #SignalLong"),
        ["Operator '#' cannot be applied to type 'Enum'."]
    );
}
