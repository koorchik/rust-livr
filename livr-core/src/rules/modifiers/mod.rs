//! Modifiers: rules that change the value and never fail. They change
//! primitive values only; lists and objects stay as they are.

/// The `default` rule.
pub mod default;
/// The `leave_only` rule.
pub mod leave_only;
/// The `remove` rule.
pub mod remove;
/// The `to_lc` rule.
pub mod to_lc;
/// The `to_uc` rule.
pub mod to_uc;
/// The `trim` rule.
pub mod trim;
