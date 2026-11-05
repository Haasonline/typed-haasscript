//! Built-in declarations: scripts checked against the built-in commands and the Lua libraries.

use ths_syntax::Dialect;

use crate::declarations::{GENERATED_DECLARATIONS, LUA_DECLARATIONS, OVERRIDE_DECLARATIONS};
use crate::test_support::{assert_clean, errors_with_built_ins, warnings_with_built_ins};
use crate::{CheckOptions, Declarations, check_with};

// Declaration files

#[test]
fn the_built_in_declarations_check_clean() {
    assert_clean(&format!("{OVERRIDE_DECLARATIONS}\n{GENERATED_DECLARATIONS}\n{LUA_DECLARATIONS}"));
}

#[test]
fn a_script_sees_the_built_in_commands() {
    assert_eq!(
        errors_with_built_ins("local average: HaasNumberCollection = SMA(ClosePrices(), 10)", Dialect::Typed),
        Vec::<String>::new()
    );
}

#[test]
fn a_plain_script_is_checked_against_the_built_in_commands() {
    assert_eq!(
        errors_with_built_ins("local average = SMA(ClosePrices(), 'ten')", Dialect::Plain),
        ["Type '\"ten\"' is not assignable to type 'number | HaasNumberCollection'."]
    );
}

#[test]
fn a_series_is_accepted_where_a_command_takes_a_number() {
    assert_eq!(
        errors_with_built_ins("local average = SMA(ClosePrices(), Floor(10.4))", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn the_built_in_overrides_replace_the_generated_signatures() {
    assert_eq!(
        errors_with_built_ins("local roll: number = Random()", Dialect::Typed),
        ["Type 'HaasNumberCollection' is not assignable to type 'number'."]
    );
}

// Lua libraries

#[test]
fn a_series_passed_to_a_math_function_is_reported() {
    assert_eq!(
        errors_with_built_ins("local whole = math.floor(SubPerc(100, 10))", Dialect::Plain),
        ["Type 'HaasNumberCollection' is not assignable to type 'number | string'."]
    );
}

#[test]
fn a_numeric_string_passed_to_a_math_function_is_accepted() {
    assert_eq!(errors_with_built_ins("local whole = math.floor('3.7')", Dialect::Plain), Vec::<String>::new());
}

#[test]
fn a_boolean_passed_to_string_format_is_reported() {
    assert_eq!(
        errors_with_built_ins("local text = string.format('%s', true)", Dialect::Plain),
        ["Type 'true' is not assignable to type 'string | number'."]
    );
}

#[test]
fn a_math_function_gives_a_number() {
    assert_eq!(errors_with_built_ins("local whole: number = math.floor(3.7)", Dialect::Typed), Vec::<String>::new());
}

#[test]
fn a_lua_library_function_is_not_a_command() {
    assert_eq!(
        warnings_with_built_ins("local count = string.len(SubPerc(100, 10))", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn diagnostics_inside_declaration_files_are_not_reported() {
    let declarations = Declarations::from_sources(&["declare function Broken(x: Missing): number"]);
    assert_eq!(check_with("local y = Broken(1)", Dialect::Typed, &declarations, CheckOptions::default()), []);
}

#[test]
fn a_position_direction_compared_with_a_signal_is_reported() {
    assert_eq!(
        errors_with_built_ins("local same = PositionContainer().direction == SignalLong", Dialect::Plain),
        [
            "This comparison appears to be unintentional because the types 'EnumPosition' and 'EnumSignal' have no overlap."
        ]
    );
}

#[test]
fn a_position_direction_compared_with_a_position_constant_is_accepted() {
    assert_eq!(
        errors_with_built_ins("local long = PositionContainer().direction == PositionLong", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn an_indicator_container_signal_compared_with_a_signal_is_accepted() {
    assert_eq!(
        errors_with_built_ins(
            "local long = IndicatorContainer(SignalLong).unanimousSignal == SignalLong",
            Dialect::Plain
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_script_redeclaring_a_built_in_type_is_reported() {
    assert_eq!(
        errors_with_built_ins("declare type HaasNumberCollection", Dialect::Typed),
        ["Duplicate identifier 'HaasNumberCollection'."]
    );
}
