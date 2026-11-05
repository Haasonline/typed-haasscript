//! Known names: reading a name nothing defines, and reads falling back past a local whose block ended.

use ths_syntax::Dialect;

use crate::test_support::{assert_clean, errors_with_built_ins, unknown_name, warnings};
use crate::{CheckOptions, Declarations, Severity, diagnose};

#[test]
fn reading_a_name_nothing_defines_is_reported() {
    assert_eq!(errors_with_built_ins("local value = Missing", Dialect::Plain), [unknown_name("Missing")]);
}

#[test]
fn a_global_assigned_later_is_known() {
    assert_eq!(
        errors_with_built_ins("local function read() return later end\nlater = 1", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn a_global_function_statement_makes_its_name_known() {
    assert_eq!(errors_with_built_ins("local f = helper\nfunction helper() end", Dialect::Plain), Vec::<String>::new());
}

#[test]
fn a_string_of_exactly_the_name_makes_it_known() {
    assert_eq!(
        errors_with_built_ins("local label = 'Hidden'\nlocal value = Hidden", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn a_comment_or_a_table_key_does_not_make_a_name_known() {
    assert_eq!(
        errors_with_built_ins("-- Noted\nlocal keyed = {Keyed = 1}\nlocal a, b = Noted, Keyed", Dialect::Plain),
        [unknown_name("Noted"), unknown_name("Keyed")]
    );
}

#[test]
fn reading_a_local_after_its_block_ended_is_reported() {
    assert_eq!(
        errors_with_built_ins("if true then local inner = 1 end\nlocal value = inner", Dialect::Plain),
        [format!("{} A local 'inner' above ended with its block.", unknown_name("inner"))]
    );
}

#[test]
fn lua_globals_the_server_knows_are_known() {
    assert_eq!(
        errors_with_built_ins("local a, b, c = type(1), tostring(2), pairs({})", Dialect::Plain),
        Vec::<String>::new()
    );
}

#[test]
fn print_is_unknown_to_the_server() {
    assert_eq!(errors_with_built_ins("print('x')", Dialect::Plain), [unknown_name("print")]);
}

#[test]
fn a_custom_command_name_is_known() {
    assert_eq!(errors_with_built_ins("local total = CC_MyCounter(1)", Dialect::Plain), Vec::<String>::new());
}

const ANY_AND_NUMBER: &str = "local flag = Input('Flag', true)\nlocal rounded = Round(flag and (2 * 3), 5)";

#[test]
fn an_any_operand_of_and_is_any_in_a_plain_file() {
    assert_eq!(errors_with_built_ins(ANY_AND_NUMBER, Dialect::Plain), Vec::<String>::new());
}

#[test]
fn an_any_operand_of_and_keeps_its_falsy_part_in_a_typed_file() {
    assert_eq!(
        errors_with_built_ins(ANY_AND_NUMBER, Dialect::Typed),
        ["Type 'nil | false | number' is not assignable to type 'number | number[] | HaasNumberCollection'."]
    );
}

#[test]
fn unknown_names_need_the_built_in_declarations() {
    assert_clean("local value = Missing");
}

fn shadowing_warnings(source: &str) -> Vec<String> {
    let options = CheckOptions { shadowing: true, ..CheckOptions::default() };
    diagnose(source, Dialect::Plain, &Declarations::default(), options)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

const SHADOWED: &str = "local price = 1\nif true then local price = 2 end\nlocal value = price";

#[test]
fn a_read_falling_back_past_an_ended_local_is_a_shadowing_warning() {
    assert_eq!(
        shadowing_warnings(SHADOWED),
        ["'price' here is the outer 'price': the local 'price' declared in a block above ended with its block."]
    );
}

#[test]
fn a_read_falling_back_past_an_ended_local_is_fine_without_the_setting() {
    assert_eq!(warnings(SHADOWED), Vec::<String>::new());
}

#[test]
fn a_local_declared_again_after_the_block_is_not_a_fallback() {
    assert_eq!(
        shadowing_warnings("if true then local price = 2 end\nlocal price = 1\nlocal value = price"),
        Vec::<String>::new()
    );
}

#[test]
fn a_read_inside_the_block_is_not_a_fallback() {
    assert_eq!(
        shadowing_warnings("local price = 1\nif true then local price = 2\nlocal value = price end"),
        Vec::<String>::new()
    );
}
