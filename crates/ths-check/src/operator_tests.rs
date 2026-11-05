//! Operators and comparisons: arithmetic, concatenation, ordering, length, and comparing types with no overlap.

use crate::test_support::{assert_clean, errors, warnings};

// Operators

#[test]
fn arithmetic_on_a_string_is_reported() {
    assert_eq!(errors("local x = 'a' - 1"), ["Operator '-' cannot be applied to types '\"a\"' and '1'."]);
}

#[test]
fn concatenation_of_a_boolean_is_reported() {
    assert_eq!(errors("local x = 'a' .. true"), ["Operator '..' cannot be applied to types '\"a\"' and 'true'."]);
}

#[test]
fn concatenation_of_numbers_and_strings_is_fine() {
    assert_clean("local x: string = 'a' .. 1 .. 'b'");
}

#[test]
fn ordering_a_number_against_a_string_is_reported() {
    assert_eq!(errors("local x = 1 < 'a'"), ["Operator '<' cannot be applied to types '1' and '\"a\"'."]);
}

#[test]
fn ordering_a_possibly_nil_value_reports_nil() {
    assert_eq!(warnings("local x: number | nil = Load('k')\nlocal y = x < 1"), ["Object is possibly 'nil'."]);
}

#[test]
fn length_of_a_number_is_reported() {
    assert_eq!(errors("local x = #5"), ["Operator '#' cannot be applied to type '5'."]);
}

// Comparisons

#[test]
fn comparing_types_with_no_overlap_is_reported() {
    assert_eq!(
        errors(
            "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\nlocal same = SignalLong == Red"
        ),
        ["This comparison appears to be unintentional because the types 'EnumSignal' and 'EnumColor' have no overlap."]
    );
}

const CONSTANTS: &str = "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\ndeclare text: string\n";

#[test]
fn comparing_a_constant_with_the_name_of_a_constant_of_its_category_is_allowed() {
    assert_clean(&format!("{CONSTANTS}local same = SignalShort ~= 'SignalLong'"));
}

#[test]
fn comparing_the_name_of_a_constant_with_a_constant_is_allowed() {
    assert_clean(&format!("{CONSTANTS}local same = 'SignalLong' == SignalShort"));
}

#[test]
fn comparing_a_constant_with_other_text_is_reported() {
    assert_eq!(
        errors(&format!("{CONSTANTS}local same = SignalLong == 'Long'")),
        ["This comparison appears to be unintentional because the types 'EnumSignal' and '\"Long\"' have no overlap."]
    );
}

#[test]
fn comparing_a_constant_with_the_name_of_a_constant_of_another_category_is_reported() {
    assert_eq!(
        errors(&format!("{CONSTANTS}local same = SignalLong == 'Red'")),
        ["This comparison appears to be unintentional because the types 'EnumSignal' and '\"Red\"' have no overlap."]
    );
}

#[test]
fn comparing_a_color_with_its_name_is_reported() {
    assert_eq!(
        errors(&format!("{CONSTANTS}local same = Red == 'Red'")),
        ["This comparison appears to be unintentional because the types 'EnumColor' and '\"Red\"' have no overlap."]
    );
}

#[test]
fn comparing_a_constant_with_a_string_variable_is_allowed() {
    assert_clean(&format!("{CONSTANTS}local same = SignalLong == text"));
}

#[test]
fn a_local_named_like_a_constant_does_not_make_its_name_text_a_constant() {
    assert_eq!(
        errors(&format!("{CONSTANTS}local Long = SignalLong\nlocal same = SignalLong == 'Long'")),
        ["This comparison appears to be unintentional because the types 'EnumSignal' and '\"Long\"' have no overlap."]
    );
}

#[test]
fn comparing_within_one_type_is_allowed() {
    assert_clean(
        "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\nlocal same = SignalLong == SignalShort",
    );
}

#[test]
fn comparing_with_a_base_type_is_allowed() {
    assert_clean(
        "declare type Enum\ndeclare type EnumSignal extends Enum\ndeclare type EnumColor extends Enum\ndeclare SignalLong: EnumSignal\ndeclare SignalShort: EnumSignal\ndeclare Red: EnumColor\ndeclare anything: Enum\nlocal same = anything == SignalLong",
    );
}

#[test]
fn comparing_with_nil_is_allowed() {
    assert_clean("local n: number = 1\nlocal missing = n == nil");
}

#[test]
fn comparing_a_number_with_a_string_is_reported() {
    assert_eq!(
        errors("local n: number = 1\nlocal same = n == 'a'"),
        ["This comparison appears to be unintentional because the types 'number' and '\"a\"' have no overlap."]
    );
}
