//! Checksum rules (LIVR 2.5): the string form is checked as it is, nothing is
//! removed; a boolean gets the error code of the rule.

/// The `credit_card` rule.
pub mod credit_card;
/// The `iban` rule.
pub mod iban;
/// The `luhn` rule.
pub mod luhn;
