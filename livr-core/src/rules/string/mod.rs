//! String rules: they accept primitive values and output strings, except `eq`
//! and `one_of`, which output the value of the rule.

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
/// The `one_of` rule.
pub mod one_of;
/// The `string` rule.
#[allow(clippy::module_inception)]
pub mod string;
