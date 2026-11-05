//! Variables: annotated and inferred locals, globals and fields, and the Lua idioms unannotated code relies on.

use ths_syntax::Dialect;

use crate::test_support::{assert_clean, errors, warnings};
use crate::{CheckOptions, Declarations, Severity, diagnose};

// Variables

#[test]
fn annotated_local_accepts_a_matching_value() {
    assert_clean("local x: number = 1");
}

#[test]
fn annotated_local_rejects_a_mismatched_value() {
    assert_eq!(errors("local x: number = 'a'"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn inferred_local_keeps_its_widened_type_for_later_assignments() {
    assert_eq!(errors("local x = 1\nx = 'a'"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

fn plain_errors(source: &str) -> Vec<String> {
    diagnose(source, Dialect::Plain, &Declarations::default(), CheckOptions::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn inferred_local_reassigned_another_kind_is_any_in_a_plain_file() {
    assert_eq!(plain_errors("local x = 1\nx = 'a'\nlocal y = x.anything"), Vec::<String>::new());
}

#[test]
fn inferred_global_reassigned_another_kind_is_any_in_a_plain_file() {
    assert_eq!(plain_errors("counter = 0\ncounter = 'a'"), Vec::<String>::new());
}

#[test]
fn inferred_field_reassigned_another_kind_is_any_in_a_plain_file() {
    assert_eq!(plain_errors("local order = {price = 1}\norder.price = 'a'"), Vec::<String>::new());
}

#[test]
fn local_initializer_reads_the_outer_variable() {
    assert_clean("local x = 'a'\ndo local x: string = x end");
}

#[test]
fn undeclared_names_are_any() {
    assert_clean("local x: number = SomeBuiltIn()");
}

#[test]
fn assignment_creates_a_typed_global() {
    assert_eq!(errors("counter = 0\ncounter = 'a'"), ["Type '\"a\"' is not assignable to type 'number'."]);
}

#[test]
fn error_in_a_local_function_signature_is_reported_once() {
    assert_eq!(errors("local function f(x: Missing) end"), ["Cannot find name 'Missing'."]);
}

#[test]
fn unknown_type_name_is_reported() {
    assert_eq!(errors("local x: Missing = 1"), ["Cannot find name 'Missing'."]);
}

// Lua idioms in code without annotations

#[test]
fn inferred_table_accepts_new_fields() {
    assert_clean("local order = {}\norder.price = 1\nlocal p = order.price");
}

#[test]
fn inferred_variable_accepts_nil() {
    assert_clean("local current = 1\ncurrent = nil");
}

#[test]
fn inferred_variable_set_to_nil_is_possibly_nil_afterwards() {
    assert_eq!(warnings("local current = 1\ncurrent = nil\nlocal next = current + 1"), ["Object is possibly 'nil'."]);
}

#[test]
fn field_initialized_to_nil_accepts_any_value_later() {
    assert_clean("local colors = { exit = nil }\ncolors.exit = Load('k')");
}

#[test]
fn field_of_an_inferred_table_accepts_nil() {
    assert_clean("local order = { price = 1 }\norder.price = nil");
}

#[test]
fn inferred_empty_table_can_be_replaced_by_an_array() {
    assert_clean("local models = {}\nmodels = { { name = 'a' }, { name = 'b' } }");
}

#[test]
fn unannotated_parameters_may_be_left_out() {
    assert_clean("local function f(a, b) end\nf(1)");
}

#[test]
fn unannotated_parameter_accepts_nil() {
    assert_clean("local function f(a) end\nf(nil)");
}

#[test]
fn unannotated_parameter_is_not_possibly_nil_inside() {
    assert_clean("local function f(a) return a + 1 end");
}
