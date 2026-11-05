//! Custom commands: the declaration a command file gives its callers, its parameters inside it, and globals it may not create.

use ths_syntax::Dialect;

use crate::{CheckOptions, Declarations, Severity, check_with};

const SCALE_COMMAND: &str = "DefineCommand('Scale', 'Converts a value to another scale.')
local value = DefineParameter(NumberType, 'value', 'The number to convert.', true, 2)
local source = DefineParameter(ListNumberType, 'source', 'The source data.', true, {1, 2, 3})
local scale = DefineParameter(NumberType, 'scale', 'The new scale.', false, 100)
DefineOutput(NumberType, value, 'The value converted to the new scale')";

fn command_text(source: &str) -> Option<String> {
    crate::command_declaration(source, Dialect::Plain, &Declarations::built_in()).map(|declaration| declaration.text)
}

/// Errors in `caller` (typed) when the project also holds `command`.
fn errors_calling(command: &str, caller: &str) -> Vec<String> {
    let declaration = crate::command_declaration(command, Dialect::Plain, &Declarations::built_in()).unwrap();
    let declarations = Declarations::built_in().with_sources(&[&declaration.text]);
    check_with(caller, Dialect::Typed, &declarations, CheckOptions::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn a_command_file_declares_its_command_for_callers() {
    assert_eq!(
        command_text(SCALE_COMMAND).unwrap(),
        "declare function CC_Scale(value: number | HaasNumberCollection, source: number[] | HaasNumberCollection, scale?: number | HaasNumberCollection): number\n"
    );
}

#[test]
fn a_file_without_define_command_declares_nothing() {
    assert_eq!(command_text("local value = DefineParameter(NumberType, 'value', 'A number', true, 1)"), None);
}

#[test]
fn a_parameter_not_written_with_literals_is_any_and_optional() {
    let command = "local kind = NumberType\nDefineCommand('Loose', 'x')\nlocal value = DefineParameter(kind, 'value', 'x', isNeeded, 1)";
    assert_eq!(command_text(command).unwrap(), "declare function CC_Loose(value?: any): nil\n");
}

#[test]
fn output_index_entries_give_a_container() {
    let command = "DefineCommand('Pair', 'x')
DefineOutput(ListDynamicType, {label = 'a', double = 2}, 'x')
DefineOutputIndex(1, StringType, 'label', 'x')
DefineOutputIndex(2, NumberType, 'double', 'x')";
    assert_eq!(
        command_text(command).unwrap(),
        "interface CC_PairResult { label: string; 1: string; double: number; 2: number; [index: number]: any }\ndeclare function CC_Pair(): CC_PairResult\n"
    );
}

#[test]
fn a_call_with_too_few_arguments_is_an_error() {
    assert_eq!(errors_calling(SCALE_COMMAND, "local scaled = CC_Scale(5)"), ["Expected 2-3 arguments, but got 1."]);
}

#[test]
fn a_call_with_the_wrong_argument_type_is_an_error() {
    assert_eq!(
        errors_calling(SCALE_COMMAND, "local scaled = CC_Scale(true, ClosePrices())"),
        ["Type 'true' is not assignable to type 'number | HaasNumberCollection'."]
    );
}

#[test]
fn a_command_result_has_its_output_type() {
    assert_eq!(
        errors_calling(SCALE_COMMAND, "local scaled: string = CC_Scale(5, ClosePrices())"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn a_container_result_is_read_by_name_and_index() {
    let command = "DefineCommand('Pair', 'x')
DefineOutput(ListDynamicType, {label = 'a', double = 2}, 'x')
DefineOutputIndex(1, StringType, 'label', 'x')
DefineOutputIndex(2, NumberType, 'double', 'x')";
    assert_eq!(
        errors_calling(command, "local pair = CC_Pair()\nlocal a: number = pair.double\nlocal b: string = pair[1]"),
        Vec::<String>::new()
    );
}

#[test]
fn a_number_for_a_command_text_parameter_is_only_a_warning() {
    let command = "DefineCommand('Shout', 'x')\nlocal text = DefineParameter(StringType, 'text', 'x', true, 'a')\nDefineOutput(StringType, text, 'x')";
    assert_eq!(errors_calling(command, "local loud = CC_Shout(5)"), Vec::<String>::new());
    let declaration = crate::command_declaration(command, Dialect::Plain, &Declarations::built_in()).unwrap();
    let declarations = Declarations::built_in().with_sources(&[&declaration.text]);
    let warnings: Vec<Severity> =
        check_with("local loud = CC_Shout(5)", Dialect::Typed, &declarations, CheckOptions::default())
            .into_iter()
            .map(|diagnostic| diagnostic.severity)
            .collect();
    assert_eq!(warnings, [Severity::Warning]);
}

#[test]
fn a_list_output_has_the_type_of_the_value_given() {
    let command = "DefineCommand('Average', 'x')\nDefineOutput(ListNumberType, SMA(ClosePrices(), 10), 'x')";
    assert_eq!(command_text(command).unwrap(), "declare function CC_Average(): HaasNumberCollection\n");
}

#[test]
fn an_enum_output_has_the_type_of_the_constant_given() {
    let command = "DefineCommand('Signal', 'x')\nDefineOutput(EnumType, SignalLong, 'x')";
    assert_eq!(command_text(command).unwrap(), "declare function CC_Signal(): EnumSignal\n");
}

#[test]
fn an_output_value_the_server_converts_keeps_the_constant_type() {
    let command = "DefineCommand('Label', 'x')\nDefineOutput(StringType, 5, 'x')";
    assert_eq!(command_text(command).unwrap(), "declare function CC_Label(): string\n");
}

#[test]
fn outputs_given_in_several_places_join_their_types() {
    let command = "DefineCommand('Either', 'x')
if IsTradeAmountEnough() then
  DefineOutput(ListNumberType, ClosePrices(), 'x')
else
  DefineOutput(ListNumberType, {1, 2}, 'x')
end";
    assert_eq!(command_text(command).unwrap(), "declare function CC_Either(): HaasNumberCollection | number[]\n");
}

#[test]
fn an_output_entry_has_the_type_of_its_table_field() {
    let command = "DefineCommand('Bands', 'x')
DefineOutput(ListDynamicType, {upper = ClosePrices(), width = 3}, 'x')
DefineOutputIndex(1, ListNumberType, 'upper', 'x')
DefineOutputIndex(2, NumberType, 'width', 'x')";
    assert_eq!(
        command_text(command).unwrap(),
        "interface CC_BandsResult { upper: HaasNumberCollection; 1: HaasNumberCollection; width: number; 2: number; [index: number]: any }\ndeclare function CC_Bands(): CC_BandsResult\n"
    );
}

#[test]
fn an_output_of_a_type_only_the_command_declares_keeps_the_constant_type() {
    let command = "type Prices = number[]
DefineCommand('Own', 'x')
local prices: Prices = {1, 2}
DefineOutput(ListNumberType, prices, 'x')";
    let declaration = crate::command_declaration(command, Dialect::Typed, &Declarations::built_in()).unwrap();
    assert_eq!(declaration.text, "declare function CC_Own(): number[] | HaasNumberCollection\n");
}

/// Errors in a typed command file, seeing the built-in declarations.
fn command_errors(command: &str) -> Vec<String> {
    check_with(command, Dialect::Typed, &Declarations::built_in(), CheckOptions::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn a_number_parameter_is_a_number_inside_the_command() {
    assert_eq!(
        command_errors(
            "local period = DefineParameter(NumberType, 'period', 'x', true, 10)\nlocal text: string = period"
        ),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn a_text_parameter_is_a_string_inside_the_command() {
    assert_eq!(
        command_errors(
            "local label = DefineParameter(StringType, 'label', 'x', true, 'a')\nlocal count: number = label"
        ),
        ["Type 'string' is not assignable to type 'number'."]
    );
}

#[test]
fn a_boolean_parameter_is_a_boolean_inside_the_command() {
    assert_eq!(
        command_errors(
            "local flag = DefineParameter(BooleanType, 'flag', 'x', true, true)\nlocal count: number = flag"
        ),
        ["Type 'boolean' is not assignable to type 'number'."]
    );
}

#[test]
fn an_enum_parameter_has_its_default_category_inside_the_command() {
    assert_eq!(
        command_errors(
            "local signal = DefineParameter(EnumType, 'signal', 'x', true, SignalLong)\nlocal count: number = signal"
        ),
        ["Type 'EnumSignal' is not assignable to type 'number'."]
    );
}

#[test]
fn an_enum_parameter_of_a_moving_average_default_takes_moving_average_types() {
    assert_eq!(
        command_errors(
            "local kind = DefineParameter(EnumType, 'kind', 'x', false, SmaType)\nlocal average = MA(ClosePrices(), 10, kind)"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_caller_passes_an_enum_parameter_of_its_default_category() {
    let command = "DefineCommand('Average', 'x')\nlocal kind = DefineParameter(EnumType, 'kind', 'x', false, SmaType)";
    assert_eq!(
        errors_calling(command, "CC_Average(SignalLong)"),
        ["Type 'EnumSignal' is not assignable to type 'EnumMovingAverages | nil'."]
    );
}

#[test]
fn is_true_takes_a_number() {
    assert_eq!(command_errors("local enabled = IsTrue(1)\nlocal disabled = IsFalse(0)"), Vec::<String>::new());
}

#[test]
fn a_list_parameter_is_a_series_or_its_default() {
    assert_eq!(
        command_errors(
            "local source = DefineParameter(ListNumberType, 'source', 'x', false, {1, 2})\nlocal count: number = source"
        ),
        ["Type 'HaasNumberCollection | number[]' is not assignable to type 'number'."]
    );
}

#[test]
fn a_list_parameter_with_a_series_default_is_a_series() {
    let command = "DefineCommand('Prices', 'x')
local source = DefineParameter(ListNumberType, 'source', 'x', false, ClosePrices())
DefineOutput(ListNumberType, source, 'x')";
    assert_eq!(
        command_text(command).unwrap(),
        "declare function CC_Prices(source?: number[] | HaasNumberCollection): HaasNumberCollection\n"
    );
}

#[test]
fn a_dynamic_parameter_can_be_anything_inside_the_command() {
    assert_eq!(
        command_errors("local value = DefineParameter(DynamicType, 'value', 'x', true, 5)\nlocal text: string = value"),
        Vec::<String>::new()
    );
}

#[test]
fn a_local_named_define_parameter_is_an_ordinary_function() {
    assert_eq!(
        command_errors("local function DefineParameter(kind: any, name: string, description: string, isRequired: boolean, default: any): any return default end
local value = DefineParameter(NumberType, 'value', 'x', true, 5)
local text: string = value"),
        Vec::<String>::new()
    );
}

const COMMAND_GLOBAL_MESSAGE: &str =
    "a custom command may not create globals, and the server refuses to compile one that does. Declare it 'local'.";

/// What a plain command file reports, as `ths check` shows it.
fn plain_command_messages(command: &str) -> Vec<String> {
    check_with(command, Dialect::Plain, &Declarations::built_in(), CheckOptions::default())
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn a_global_assigned_in_a_command_is_reported_even_in_a_plain_file() {
    assert_eq!(
        plain_command_messages("DefineCommand('Counter', 'x')\ntotal = 1"),
        [format!("Cannot assign to 'total': {COMMAND_GLOBAL_MESSAGE}")]
    );
}

#[test]
fn a_global_assigned_in_an_uncalled_function_of_a_command_is_reported() {
    assert_eq!(
        plain_command_messages("DefineCommand('Counter', 'x')\nlocal function never() total = 1 end"),
        [format!("Cannot assign to 'total': {COMMAND_GLOBAL_MESSAGE}")]
    );
}

#[test]
fn a_global_function_statement_in_a_command_is_reported() {
    assert_eq!(
        plain_command_messages("DefineCommand('Counter', 'x')\nfunction helper() return 1 end"),
        [format!("Cannot assign to 'helper': {COMMAND_GLOBAL_MESSAGE}")]
    );
}

#[test]
fn a_global_assigned_twice_in_a_command_is_reported_once() {
    assert_eq!(
        plain_command_messages("DefineCommand('Counter', 'x')\ntotal = 1\ntotal = 2"),
        [format!("Cannot assign to 'total': {COMMAND_GLOBAL_MESSAGE}")]
    );
}

#[test]
fn locals_fields_and_built_in_names_may_be_assigned_in_a_command() {
    let command = "DefineCommand('Counter', 'x')
local count = 1
count = count + 1
local helper
function helper() return 1 end
string.extra = 1
Log = Log";
    assert_eq!(plain_command_messages(command), Vec::<String>::new());
}

#[test]
fn a_script_that_defines_no_command_may_assign_globals() {
    assert_eq!(plain_command_messages("total = 1\nfunction helper() return 1 end"), Vec::<String>::new());
}
