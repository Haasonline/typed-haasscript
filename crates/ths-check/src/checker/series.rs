//! Operator rules for the engine's series, each verified on the
//! TradeServer. `series[i]` is the series from element i onward, and
//! `+ - * / %` and unary minus work element-wise with a series on either
//! side, giving a series. `^` fails on a series, and a boolean operand
//! aborts the script. A series is read-only: `series[i] = v` raises an
//! error. Comparisons use the newest element, so a series may be ordered
//! against and equal a number; against anything else a comparison is
//! silently false. `..` gives the newest element as text only next to a
//! string.

use ths_syntax::SyntaxNode;

use crate::engine_types::SERIES_TYPE;
use crate::types::Type;

use super::Checker;

const SERIES_CONCATENATION_MESSAGE: &str =
    "Concatenating a series with a number or another series loses one side; concatenate it with a string.";

pub(super) const SERIES_READ_ONLY_MESSAGE: &str = "A series is read-only: use 'ArrayUnshift(series, value)' to add a newest value or 'ArrayAdd(series, value)' to add an oldest one.";

/// Whether an expanded type is the series.
pub(super) fn is_series(value: &Type) -> bool {
    matches!(value, Type::Opaque(name) if name == SERIES_TYPE)
}

/// What the other operand of series arithmetic may be. A string is
/// converted to a number at runtime.
pub(super) fn is_series_operand(value: &Type) -> bool {
    is_series(value) || matches!(value, Type::Number | Type::NumberLiteral(_) | Type::String | Type::StringLiteral(_))
}

fn is_number(value: &Type) -> bool {
    matches!(value, Type::Number | Type::NumberLiteral(_))
}

fn is_string(value: &Type) -> bool {
    matches!(value, Type::String | Type::StringLiteral(_))
}

/// Whether two expanded types can compare equal because one is a series:
/// a series compares by its newest element, so it may equal a number.
pub(super) fn series_may_equal(left: &Type, right: &Type) -> bool {
    let is_number = |value: &Type| matches!(value, Type::Number | Type::NumberLiteral(_));
    (is_series(left) && is_number(right)) || (is_number(left) && is_series(right))
}

pub(super) fn series_type() -> Type {
    Type::Opaque(SERIES_TYPE.to_string())
}

impl Checker<'_> {
    /// Whether some member of `value` is the series.
    pub(super) fn may_be_series(&self, value: &Type) -> bool {
        self.expand(&value.without_nil()).members().iter().any(|member| is_series(&self.expand(member)))
    }

    /// `..` with a series gives its newest element as text only when the
    /// other side is a string; with a number or another series the server
    /// silently drops one side. Nil is reported by the nil check.
    pub(super) fn check_series_concatenation(
        &mut self,
        (left, left_type): (&SyntaxNode, &Type),
        (right, right_type): (&SyntaxNode, &Type),
    ) {
        let mut reported_nil = false;
        for (operand, value) in [(left, left_type), (right, right_type)] {
            if value.may_be_nil() {
                self.report_possibly_nil(operand.text_range());
                reported_nil = true;
            }
        }
        if reported_nil {
            return;
        }
        let members = |value: &Type| -> Vec<Type> {
            self.expand(value).members().iter().map(|member| self.expand(member)).collect()
        };
        let (left_members, right_members) = (members(left_type), members(right_type));
        let fits = left_members.iter().all(|left| right_members.iter().all(|right| concatenation_fits(left, right)));
        if fits {
            return;
        }
        let range = left.text_range().cover(right.text_range());
        let both_numeric = self.all_members(left_type, |member| is_series(member) || is_number(member))
            && self.all_members(right_type, |member| is_series(member) || is_number(member));
        let message = if both_numeric {
            SERIES_CONCATENATION_MESSAGE.to_string()
        } else {
            format!("Operator '..' cannot be applied to types '{left_type}' and '{right_type}'.")
        };
        self.sink.error(range, message);
    }

    /// Whether every member of `value`, expanded, satisfies `accepts`.
    fn all_members(&self, value: &Type, accepts: impl Fn(&Type) -> bool) -> bool {
        self.expand(value).members().iter().all(|member| accepts(&self.expand(member)))
    }

    /// Whether `value` can be ordered against a series (`<` and friends
    /// compare newest elements). Anything else compares silently false.
    pub(super) fn is_series_comparand(&self, value: &Type) -> bool {
        self.expand(value).members().iter().all(|member| {
            let member = self.expand(member);
            member.is_any() || is_series(&member) || matches!(member, Type::Number | Type::NumberLiteral(_))
        })
    }

    /// Whether some member of `value` is something other than the series.
    fn may_be_other_than_series(&self, value: &Type) -> bool {
        self.expand(&value.without_nil()).members().iter().any(|member| !is_series(&self.expand(member)))
    }

    /// The result of arithmetic on `operands` when one may be a series: a
    /// series, or also a number when every operand may be something else.
    pub(super) fn series_arithmetic_result(&self, operands: &[&Type]) -> Type {
        if operands.iter().all(|operand| self.may_be_other_than_series(operand)) {
            Type::union([Type::Number, series_type()])
        } else {
            series_type()
        }
    }
}

/// Whether `left .. right` works for one kind of value on each side: a
/// series only beside text (beside a number or another series the server
/// silently drops one side), a number beside text or a number.
fn concatenation_fits(left: &Type, right: &Type) -> bool {
    let is_text = |value: &Type| value.is_any() || is_string(value);
    if is_text(left) {
        is_text(right) || is_number(right) || is_series(right)
    } else if is_text(right) {
        is_number(left) || is_series(left)
    } else {
        is_number(left) && is_number(right)
    }
}
