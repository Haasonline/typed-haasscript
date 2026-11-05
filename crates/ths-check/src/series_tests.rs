//! Series: number variables widened by a series, series keys, and operators, comparisons and concatenation on a series.

use crate::test_support::{assert_clean, errors};

const SERIES: &str = "declare type HaasNumberCollection\ndeclare close: HaasNumberCollection\ndeclare flag: boolean\ndeclare mixed: number | HaasNumberCollection\n";

fn series_errors(body: &str) -> Vec<String> {
    errors(&format!("{SERIES}{body}"))
}

#[test]
fn an_inferred_number_variable_accepts_a_series() {
    assert_eq!(series_errors("local total = 0\ntotal = close"), Vec::<String>::new());
}

#[test]
fn an_inferred_number_variable_holding_a_series_is_reported_as_a_for_bound() {
    assert_eq!(
        series_errors("local total = 0\ntotal = close\nfor i = 1, total do end"),
        ["Type 'HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn an_inferred_number_variable_widened_by_a_series_may_hold_either_after_a_branch() {
    assert_eq!(
        series_errors("local total = 0\nif flag then total = close end\nfor i = 1, total do end"),
        ["Type 'number | HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn a_counter_that_only_holds_numbers_stays_a_number() {
    assert_eq!(series_errors("local n = 0\nfor i = 1, 3 do n = n + 1 end\nfor j = 1, n do end"), Vec::<String>::new());
}

#[test]
fn an_inferred_series_variable_accepts_a_number() {
    assert_eq!(series_errors("local average = close\naverage = 0"), Vec::<String>::new());
}

#[test]
fn an_inferred_number_field_accepts_a_series() {
    assert_eq!(
        series_errors("local state = {change = 0}\nstate.change = close\nstate.change = -state.change"),
        Vec::<String>::new()
    );
}

#[test]
fn a_nested_inferred_number_field_accepts_a_series() {
    assert_eq!(
        series_errors(
            "local this = {state = {change = 0}}\nthis.state.change = close\nthis.state.change = -this.state.change"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn an_inferred_number_field_holding_a_series_is_reported_as_a_for_bound() {
    assert_eq!(
        series_errors(
            "local state = {change = 0}\nif flag then state.change = close end\nfor i = 1, state.change do end"
        ),
        ["Type 'number | HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn an_inferred_series_field_accepts_a_number() {
    assert_eq!(series_errors("local state = {average = close}\nstate.average = 0"), Vec::<String>::new());
}

#[test]
fn a_field_of_an_annotated_table_keeps_its_type() {
    assert_eq!(
        series_errors("local state: {change: number} = {change = 0}\nstate.change = close"),
        ["Type 'HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn a_number_or_series_concatenated_with_text_is_accepted() {
    assert_eq!(series_errors("local text = 'v=' .. mixed .. ''"), Vec::<String>::new());
}

#[test]
fn a_number_or_series_concatenated_with_a_number_is_reported() {
    assert_eq!(
        series_errors("local text = mixed .. 1"),
        ["Concatenating a series with a number or another series loses one side; concatenate it with a string."]
    );
}

const SERIES_KEY: &str = "A series used as a table key is a key of its own, not its number, so the entry is never found; use ArrayGet(value, 1) for the number.";

#[test]
fn a_series_key_read_from_a_table_is_reported() {
    assert_eq!(series_errors("local t = {a = 1}\nlocal v = t[close]"), [SERIES_KEY]);
}

#[test]
fn a_series_key_written_to_a_table_is_reported() {
    assert_eq!(series_errors("local t = {}\nt[close] = 1"), [SERIES_KEY]);
}

#[test]
fn a_series_key_written_to_a_list_is_reported() {
    assert_eq!(series_errors("local t: string[] = {}\nt[close] = 'a'"), [SERIES_KEY]);
}

#[test]
fn a_key_that_may_be_a_series_is_reported() {
    assert_eq!(series_errors("local t = {}\nt[mixed] = 1"), [SERIES_KEY]);
}

#[test]
fn a_number_key_is_accepted() {
    assert_eq!(series_errors("local t = {}\nt[1] = 1\nlocal v = t[2]"), Vec::<String>::new());
}

#[test]
fn an_annotated_number_variable_still_rejects_a_series() {
    assert_eq!(
        series_errors("local total: number = 0\ntotal = close"),
        ["Type 'HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn an_inferred_number_variable_still_rejects_a_string() {
    assert_eq!(series_errors("local total = 0\ntotal = 'a'"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn indexing_a_series_gives_a_series() {
    assert_eq!(
        series_errors("local x: number = close[3]"),
        ["Type 'HaasNumberCollection' is not assignable to type 'number'."]
    );
}

#[test]
fn a_series_is_indexed_by_a_number() {
    assert_eq!(series_errors("local x = close['a']"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn assigning_into_a_series_is_reported() {
    assert_eq!(
        series_errors("close[1] = 5"),
        [
            "A series is read-only: use 'ArrayUnshift(series, value)' to add a newest value or 'ArrayAdd(series, value)' to add an oldest one."
        ]
    );
}

#[test]
fn arithmetic_with_a_series_on_either_side_gives_a_series() {
    assert_clean(&format!(
        "{SERIES}local a: HaasNumberCollection = close * 10\nlocal b: HaasNumberCollection = 10 - close\nlocal c: HaasNumberCollection = close % 2\nlocal d: HaasNumberCollection = close[3] / close"
    ));
}

#[test]
fn arithmetic_with_a_series_accepts_a_string() {
    assert_clean(&format!("{SERIES}local a: HaasNumberCollection = close + '1'"));
}

#[test]
fn arithmetic_with_a_series_rejects_a_boolean() {
    assert_eq!(
        series_errors("local a = close + flag"),
        ["Operator '+' cannot be applied to types 'HaasNumberCollection' and 'boolean'."]
    );
}

#[test]
fn power_of_a_series_is_reported() {
    assert_eq!(
        series_errors("local a = close ^ 2"),
        ["Operator '^' cannot be applied to types 'HaasNumberCollection' and '2'."]
    );
}

#[test]
fn negating_a_series_gives_a_series() {
    assert_clean(&format!("{SERIES}local a: HaasNumberCollection = -close"));
}

#[test]
fn arithmetic_on_a_value_that_may_be_a_series_may_give_a_number() {
    assert_eq!(
        series_errors("local a: HaasNumberCollection = mixed * 2"),
        ["Type 'number | HaasNumberCollection' is not assignable to type 'HaasNumberCollection'."]
    );
}

#[test]
fn a_series_is_ordered_against_a_number_on_either_side() {
    assert_clean(&format!("{SERIES}local a = close > 100\nlocal b = 7 <= close[2]\nlocal c = close < close[3]"));
}

#[test]
fn ordering_a_series_against_a_string_is_reported() {
    assert_eq!(
        series_errors("local a = close < '8'"),
        ["Operator '<' cannot be applied to types 'HaasNumberCollection' and '\"8\"'."]
    );
}

#[test]
fn a_series_may_equal_a_number() {
    assert_clean(&format!(
        "{SERIES}local a = close == 7\nlocal b = 7 ~= close\nlocal c = close == close[2]\nlocal d = close ~= nil"
    ));
}

#[test]
fn comparing_a_series_with_a_boolean_is_reported() {
    assert_eq!(
        series_errors("local a = close == flag"),
        [
            "This comparison appears to be unintentional because the types 'HaasNumberCollection' and 'boolean' have no overlap."
        ]
    );
}

#[test]
fn a_series_concatenates_with_a_string_on_either_side() {
    assert_clean(&format!("{SERIES}local a: string = 'close ' .. close\nlocal b: string = close[2] .. ' before'"));
}

#[test]
fn concatenating_a_series_with_a_number_is_reported() {
    assert_eq!(
        series_errors("local a = close .. 1"),
        ["Concatenating a series with a number or another series loses one side; concatenate it with a string."]
    );
}

#[test]
fn concatenating_two_series_is_reported() {
    assert_eq!(
        series_errors("local a = close .. close[2]"),
        ["Concatenating a series with a number or another series loses one side; concatenate it with a string."]
    );
}

#[test]
fn the_inner_concatenation_of_a_chain_is_checked() {
    assert_eq!(
        series_errors("local a = 'x ' .. close .. 1"),
        ["Concatenating a series with a number or another series loses one side; concatenate it with a string."]
    );
}

#[test]
fn concatenating_a_series_with_a_boolean_is_reported() {
    assert_eq!(
        series_errors("local a = close .. flag"),
        ["Operator '..' cannot be applied to types 'HaasNumberCollection' and 'boolean'."]
    );
}

#[test]
fn the_length_of_a_series_is_a_number() {
    assert_clean(&format!("{SERIES}local count: number = #close + #close[3]"));
}
