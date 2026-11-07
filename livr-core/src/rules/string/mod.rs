//! String rules: they accept primitive values and output strings, except `eq`,
//! `one_of` and `not_one_of`, which output the value.

/// The `contains` rule.
pub mod contains;
/// The `digits` rule.
pub mod digits;
/// The `ends_with` rule.
pub mod ends_with;
/// The `eq` rule.
pub mod eq;
/// The `length_between` rule.
pub mod length_between;
/// The `length_equal` rule.
pub mod length_equal;
/// The `like` rule.
pub mod like;
/// The `max_length` rule.
pub mod max_length;
/// The `min_length` rule.
pub mod min_length;
/// The `not_one_of` rule.
pub mod not_one_of;
/// The `one_of` rule.
pub mod one_of;
/// The `starts_with` rule.
pub mod starts_with;
/// The `string` rule.
#[allow(clippy::module_inception)]
pub mod string;
