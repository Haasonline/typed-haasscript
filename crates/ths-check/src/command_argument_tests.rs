//! Arguments to commands: named-argument tables, conversion to text, and containers used by their first field.

use ths_syntax::Dialect;

use crate::test_support::{assert_clean, errors, errors_with_built_ins, warnings, warnings_with_built_ins};

// Containers used by their first field

#[test]
fn current_price_for_a_command_number_parameter_is_a_warning() {
    let source = "local cut = SubPerc(100, CurrentPrice())";
    assert_eq!(errors_with_built_ins(source, Dialect::Plain), Vec::<String>::new());
    assert_eq!(
        warnings_with_built_ins(source, Dialect::Plain),
        [
            "Type 'CurrentPriceResult' is not assignable to type 'number | HaasNumberCollection': the server uses its first field, the open price; pass '.close', '.bid' or '.ask' if that is not what you mean."
        ]
    );
}

#[test]
fn a_trailing_call_to_a_command_string_parameter_converts_to_text() {
    assert_eq!(
        warnings_with_built_ins("local found = StringContains('abc', GetPositionEnterPrice())", Dialect::Plain),
        [
            "Type 'number' is not assignable to type 'string': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
        ]
    );
}

#[test]
fn a_container_with_a_series_first_field_for_a_command_number_parameter_is_a_warning() {
    let source = "local cut = SubPerc(100, MACD(ClosePrices(), 12, 26, 9))";
    assert_eq!(errors_with_built_ins(source, Dialect::Plain), Vec::<String>::new());
    assert_eq!(
        warnings_with_built_ins(source, Dialect::Plain),
        [
            "Type 'MACDResult' is not assignable to type 'number | HaasNumberCollection': the server uses its first field; pass the field you mean."
        ]
    );
}

#[test]
fn a_container_with_a_number_first_field_for_a_command_number_parameter_is_a_warning() {
    assert_eq!(errors_with_built_ins("local cut = SubPerc(Balance(), 10)", Dialect::Plain), Vec::<String>::new());
}

#[test]
fn a_container_with_a_boolean_first_field_for_a_command_number_parameter_stays_an_error() {
    assert_eq!(
        errors_with_built_ins("local cut = SubPerc(100, Compare(2, 1))", Dialect::Plain),
        ["Type 'CompareResult' is not assignable to type 'number | HaasNumberCollection'."]
    );
}

#[test]
fn a_script_interface_for_a_command_number_parameter_stays_an_error() {
    assert_eq!(
        errors_with_built_ins(
            "interface Pair { 1: number }\ndeclare function MakePair(): Pair\nlocal cut = SubPerc(100, MakePair())",
            Dialect::Typed
        ),
        ["Type 'Pair' is not assignable to type 'number | HaasNumberCollection'."]
    );
}

#[test]
fn current_price_for_a_number_variable_stays_an_error() {
    assert_eq!(
        errors_with_built_ins("local price: number = CurrentPrice()", Dialect::Typed),
        ["Type 'CurrentPriceResult' is not assignable to type 'number'."]
    );
}

#[test]
fn current_price_for_a_lua_function_number_parameter_stays_an_error() {
    assert_eq!(
        errors_with_built_ins("local function f(n: number) end\nf(CurrentPrice())", Dialect::Typed),
        ["Type 'CurrentPriceResult' is not assignable to type 'number'."]
    );
}

#[test]
fn current_price_for_a_command_named_number_argument_is_a_warning() {
    assert_eq!(
        warnings_with_built_ins("local id = PlaceBuyOrder(100, 1, {triggerPrice = CurrentPrice()})", Dialect::Plain),
        [
            "Type 'CurrentPriceResult' is not assignable to type 'number | HaasNumberCollection | nil': the server uses its first field, the open price; pass '.close', '.bid' or '.ask' if that is not what you mean."
        ]
    );
}

#[test]
fn a_current_price_field_where_a_number_is_expected_is_accepted() {
    assert_eq!(
        errors_with_built_ins("local cut = SubPerc(100, CurrentPrice().close)", Dialect::Plain),
        Vec::<String>::new()
    );
}

// Named-argument tables

const ORDER: &str = "declare type EnumOrderTypes\ndeclare MarketOrderType: EnumOrderTypes\ndeclare function PlaceOrder(price: number, amount: number, market?: string, type?: EnumOrderTypes, note?: string): string\n";

#[test]
fn a_named_argument_table_fills_later_optional_parameters() {
    assert_clean(&format!("{ORDER}PlaceOrder(1, 2, {{type = MarketOrderType, note = 'x'}})"));
}

#[test]
fn a_named_argument_is_checked_against_its_parameter() {
    assert_eq!(
        errors(&format!("{ORDER}PlaceOrder(1, 2, {{type = 5}})")),
        ["Type '5' is not assignable to type 'EnumOrderTypes | nil'."]
    );
}

#[test]
fn a_table_whose_keys_match_no_later_parameter_is_reported() {
    assert_eq!(
        errors(&format!("{ORDER}PlaceOrder(1, 2, {{nte = 'x'}})")),
        [
            "Named argument 'nte' matches no parameter from 'market' on, and no key here matches: the server reads the table as 'market'."
        ]
    );
}

#[test]
fn a_table_whose_only_key_names_an_earlier_parameter_is_reported() {
    assert_eq!(
        errors(&format!("{ORDER}PlaceOrder(1, 2, 'm', {{amount = 3}})")),
        [
            "Named argument 'amount' names a parameter before 'type', and no key here matches: the server reads the table as 'type'."
        ]
    );
}

#[test]
fn an_unmatched_key_beside_a_matching_one_is_an_ignored_warning() {
    assert_eq!(
        warnings(&format!("{ORDER}PlaceOrder(1, 2, {{price = 3, note = 'x'}})")),
        ["Named argument 'price' is ignored: it names a parameter before 'market'."]
    );
}

#[test]
fn a_named_argument_table_in_a_required_slot_is_reported() {
    assert_eq!(
        errors(&format!("{ORDER}PlaceOrder(1, {{market = 'm'}})")),
        [
            "A table of named arguments works only in an optional parameter's slot: the server reads this one as 'amount' and stops the script."
        ]
    );
}

#[test]
fn a_table_for_a_parameter_that_takes_tables_is_an_ordinary_argument() {
    assert_clean("declare function Plot(value: number, options?: any): nil\nPlot(1, {color = 'red', id = 'x'})");
}

#[test]
fn a_named_argument_table_must_be_the_last_argument() {
    assert_eq!(
        warnings(&format!("{ORDER}PlaceOrder(1, 2, {{note = 'x'}}, MarketOrderType)")),
        [
            "Type '{ note: string }' is not assignable to type 'string | nil': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
        ]
    );
}

#[test]
fn a_color_takes_its_opacity_as_a_named_argument_table() {
    assert_eq!(
        errors_with_built_ins("local a = Aqua({opacity = 20})\nlocal b = Aqua({opacity = 'x'})", Dialect::Typed),
        ["Type '\"x\"' is not assignable to type 'number'."]
    );
}

#[test]
fn a_named_argument_table_may_be_held_in_a_variable() {
    assert_clean(&format!("{ORDER}local options = {{type = MarketOrderType, note = 'x'}}\nPlaceOrder(1, 2, options)"));
}

#[test]
fn a_held_named_argument_table_is_checked() {
    assert_eq!(
        warnings(&format!("{ORDER}local options = {{nte = 'x', note = 'y'}}\nPlaceOrder(1, 2, options)")),
        ["Named argument 'nte' is ignored: it matches no parameter from 'market' on."]
    );
}

const ORDER_OPTIONS: &str = "local function options() return {type = MarketOrderType, note = 'x'} end\n";

#[test]
fn a_named_argument_table_may_be_returned_by_a_call() {
    assert_clean(&format!("{ORDER}{ORDER_OPTIONS}PlaceOrder(1, 2, options())"));
}

#[test]
fn a_returned_named_argument_table_is_checked() {
    assert_eq!(
        warnings(&format!(
            "{ORDER}local function options() return {{nte = 'x', note = 'y'}} end\nPlaceOrder(1, 2, options())"
        )),
        ["Named argument 'nte' is ignored: it matches no parameter from 'market' on."]
    );
}

#[test]
fn a_returned_named_argument_table_in_a_required_slot_is_reported() {
    assert_eq!(
        errors(&format!("{ORDER}{ORDER_OPTIONS}PlaceOrder(1, options())")),
        [
            "A table of named arguments works only in an optional parameter's slot: the server reads this one as 'amount' and stops the script."
        ]
    );
}

#[test]
fn a_table_returned_beside_other_values_is_not_named_arguments() {
    assert_eq!(
        warnings(&format!(
            "{ORDER}local function options() return {{note = 'x'}}, 'y' end\nPlaceOrder(1, 2, options())"
        )),
        [
            "Type '{ note: string }' is not assignable to type 'string | nil': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
        ]
    );
}

// Text conversion

#[test]
fn a_command_converts_a_number_for_a_string_parameter_with_a_warning() {
    let source = "declare function StringContains(value: string, searchValue: string): boolean\nlocal found = StringContains('x12.5', 12.5)";
    assert_eq!(errors(source), Vec::<String>::new());
    assert_eq!(
        warnings(source),
        [
            "Type '12.5' is not assignable to type 'string': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
        ]
    );
}

#[test]
fn a_command_converts_a_constant_for_a_string_parameter_with_a_warning() {
    let source = "declare type EnumSignal\ndeclare SignalLong: EnumSignal\ndeclare function StringContains(value: string, searchValue: string): boolean\nlocal found = StringContains('is SignalLong', SignalLong)";
    assert_eq!(errors(source), Vec::<String>::new());
    assert_eq!(warnings(source).len(), 1);
}

#[test]
fn a_command_converts_a_named_argument_for_a_string_parameter_with_a_warning() {
    let source = format!("{ORDER}PlaceOrder(1, 2, {{note = 5}})");
    assert_eq!(errors(&source), Vec::<String>::new());
    assert_eq!(
        warnings(&source),
        [
            "Type '5' is not assignable to type 'string | nil': the server converts it to text (a constant to its name, 0.00001 to '1E-05')."
        ]
    );
}

#[test]
fn a_lua_function_given_a_number_for_a_string_parameter_is_an_error() {
    assert_eq!(
        errors("local function shout(text: string): string return text end\nlocal loud = shout(5)"),
        ["Type '5' is not assignable to type 'string'."]
    );
}
