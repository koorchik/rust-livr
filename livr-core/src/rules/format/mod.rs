//! Format rules (LIVR 2.5): they check the string form of strings and
//! numbers and output it; a boolean gets the error code of the rule.
//! Grammars match the whole string, with ASCII classes, checked by loops over
//! bytes.

/// The `base64` rule.
pub mod base64;
/// The `cidr` rule.
pub mod cidr;
/// The `digest` rule.
pub mod digest;
/// The `e164` rule.
pub mod e164;
/// The `hex` rule.
pub mod hex;
/// The `hex_color` rule.
pub mod hex_color;
/// The `hostname` rule.
pub mod hostname;
/// The `ip` rule.
pub mod ip;
/// The `jwt` rule.
pub mod jwt;
/// The `mac` rule.
pub mod mac;
/// The `mongo_id` rule.
pub mod mongo_id;
/// The `semver` rule.
pub mod semver;
/// The `uuid` rule.
pub mod uuid;
