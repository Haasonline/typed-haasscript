//! Nil: where nil is accepted, possibly-nil values, and narrowing nil out by conditions.

use crate::test_support::{assert_clean, errors, warnings};

#[test]
fn nil_is_not_assignable_to_number() {
    assert_eq!(errors("local x: number = nil"), ["Type 'nil' is not assignable to type 'number'."]);
}

#[test]
fn nil_is_not_assignable_to_any() {
    assert_eq!(errors("local x: any = nil"), ["Type 'nil' is not assignable to type 'any'."]);
}

#[test]
fn possibly_nil_operand_is_reported() {
    assert_eq!(warnings("local x: number | nil = Load('k')\nlocal y = x + 1"), ["Object is possibly 'nil'."]);
}

#[test]
fn possibly_nil_value_is_named_in_the_mismatch() {
    assert_eq!(
        warnings("local x: number | nil = Load('k')\nlocal y: number = x"),
        ["Type 'number | nil' is not assignable to type 'number': the value may be nil."]
    );
}

#[test]
fn if_on_the_variable_narrows_out_nil() {
    assert_clean("local x: number | nil = Load('k')\nif x then local y = x + 1 end");
}

#[test]
fn comparison_with_nil_narrows_out_nil() {
    assert_clean("local x: number | nil = Load('k')\nif x ~= nil then local y = x + 1 end");
}

#[test]
fn bang_equals_nil_narrows_like_tilde_equals() {
    assert_clean("local x: number | nil = Load('k')\nif nil != x then local y = x + 1 end");
}

#[test]
fn else_branch_sees_the_opposite_narrowing() {
    assert_eq!(
        warnings("local x: number | nil = Load('k')\nif x then else local y = x + 1 end"),
        ["Object is possibly 'nil'."]
    );
}

#[test]
fn early_return_narrows_the_rest_of_the_block() {
    assert_clean("local function f(x: number | nil): number\n  if not x then return 0 end\n  return x + 1\nend");
}

#[test]
fn and_narrows_its_right_operand() {
    assert_clean("local x: number | nil = Load('k')\nlocal y = x and x + 1");
}

#[test]
fn or_supplies_a_default() {
    assert_clean("local x: number | nil = Load('k')\nlocal y: number = x or 0");
}

#[test]
fn narrowing_ends_after_the_if() {
    assert_eq!(
        warnings("local x: number | nil = Load('k')\nif x then end\nlocal y = x + 1"),
        ["Object is possibly 'nil'."]
    );
}

#[test]
fn assignment_inside_a_branch_drops_the_narrowing() {
    assert_eq!(
        warnings("local x: number | nil = 1\nif Flag() then x = nil end\nlocal y = x + 1"),
        ["Object is possibly 'nil'."]
    );
}

#[test]
fn field_comparison_with_nil_narrows_the_field() {
    assert_clean(
        "interface Models { amount?: number }\nlocal function f(models: Models) if models.amount ~= nil then local x = -models.amount end end",
    );
}

#[test]
fn field_truthiness_narrows_the_field() {
    assert_clean(
        "interface Models { amount?: number }\nlocal function f(models: Models) if models.amount then local x = models.amount + 1 end end",
    );
}

#[test]
fn assigning_the_variable_drops_its_field_narrowings() {
    assert_eq!(
        warnings(
            "interface Models { amount?: number }\nlocal function f(models: Models, other: Models)\n  if models.amount then\n    models = other\n    local x = models.amount + 1\n  end\nend"
        ),
        ["Object is possibly 'nil'."]
    );
}

#[test]
fn narrowing_by_initialization() {
    assert_clean("local x: number | nil = 1\nlocal y = x + 1");
}

#[test]
fn narrowing_does_not_reach_into_a_function_body() {
    assert_eq!(
        warnings("local x: number | nil = Load('k')\nif x then\n  local f = function() return x + 1 end\nend"),
        ["Object is possibly 'nil'."]
    );
}
