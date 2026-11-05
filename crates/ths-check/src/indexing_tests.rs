//! Indexing: values that cannot be indexed, and containers read by literal or computed index.

use crate::test_support::{assert_clean, errors, warnings};

// Indexing

const PRICE_COMMAND: &str = "declare function CurrentPrice(): { close: number }\n";

#[test]
fn indexing_a_command_name_is_reported() {
    assert_eq!(
        errors(&format!("{PRICE_COMMAND}local price = CurrentPrice[1]")),
        [
            "A value of type '() => { close: number }' cannot be indexed: the server stops the script. A command's name is a function; call it first."
        ]
    );
}

#[test]
fn indexing_a_number_is_reported() {
    assert_eq!(
        errors("local count = 5\nlocal first = count[1]"),
        ["A value of type 'number' cannot be indexed: the server stops the script."]
    );
}

#[test]
fn indexing_a_boolean_is_reported() {
    assert_eq!(
        errors("local flag = true\nlocal first = flag[1]"),
        ["A value of type 'boolean' cannot be indexed: the server stops the script."]
    );
}

#[test]
fn writing_through_an_index_of_a_command_name_is_reported() {
    assert_eq!(
        errors(&format!("{PRICE_COMMAND}CurrentPrice[1] = 2")),
        [
            "A value of type '() => { close: number }' cannot be indexed: the server stops the script. A command's name is a function; call it first."
        ]
    );
}

#[test]
fn indexing_a_string_is_allowed() {
    assert_clean("local text = 'abc'\nlocal first = text[1]");
}

#[test]
fn an_inferred_variable_assigned_an_unchecked_value_may_hold_anything() {
    assert_clean(
        "local lookup = {}\nlocal result = 0\nresult = lookup.anything()\nlocal first = result[2]\nlocal field = result.close",
    );
}

#[test]
fn an_annotated_variable_assigned_an_unchecked_value_keeps_its_type() {
    assert_eq!(
        errors("local lookup = {}\nlocal result: number = 0\nresult = lookup.anything()\nlocal field = result.close"),
        ["Property 'close' does not exist on type 'number'."]
    );
}

#[test]
fn indexing_a_value_that_may_be_a_table_is_allowed() {
    assert_clean("local function pick(value: number | { [index: number]: number }) return value[1] end");
}

// Containers read by index

const POSITION: &str = "interface Position { market: string; isLong: boolean; 1: string; 2: boolean; [index: number]: string | boolean }\ndeclare position: Position\n";

#[test]
fn a_literal_index_gives_its_field_type() {
    assert_clean(&format!("{POSITION}local market: string = position[1]\nlocal isLong: boolean = position[2]"));
}

#[test]
fn a_literal_index_with_a_wrong_target_type_is_reported() {
    assert_eq!(
        errors(&format!("{POSITION}local market: boolean = position[1]")),
        ["Type 'string' is not assignable to type 'boolean'."]
    );
}

#[test]
fn a_computed_index_gives_the_index_signature_type() {
    assert_eq!(
        errors(&format!("{POSITION}local i = 1\nlocal market: string = position[i]")),
        ["Type 'string | boolean' is not assignable to type 'string'."]
    );
}

#[test]
fn a_possible_nil_is_a_warning_not_an_error() {
    let source = "local function f(x?: number): number return x + 1 end";
    assert_eq!(errors(source), Vec::<String>::new());
    assert_eq!(warnings(source), ["Object is possibly 'nil'."]);
}
