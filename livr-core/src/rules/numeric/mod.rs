//! Numeric rules: they accept numbers and strings of decimal numbers, and
//! output numbers. Booleans are not numbers.

/// The `decimal` rule.
pub mod decimal;
/// The `integer` rule.
pub mod integer;
/// The `max_number` rule.
pub mod max_number;
/// The `min_number` rule.
pub mod min_number;
/// The `number_between` rule.
pub mod number_between;
/// The `positive_decimal` rule.
pub mod positive_decimal;
/// The `positive_integer` rule.
pub mod positive_integer;
