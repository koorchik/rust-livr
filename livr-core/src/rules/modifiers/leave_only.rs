use std::collections::HashSet;

use crate::rules::prelude::*;

/// `leave_only`: removes all characters but the listed ones. The argument is a list of characters, not
/// a pattern: `"a-z"` is the three characters `a`, `-` and `z`.
#[derive(Clone, Debug)]
pub struct LeaveOnly {
    // ASCII characters are looked up in a table, others in a set
    ascii: [bool; 128],
    other: HashSet<char>,
}

impl LeaveOnly {
    /// The rule for the characters.
    #[must_use]
    pub fn new(chars: &str) -> Self {
        let mut ascii = [false; 128];
        let mut other = HashSet::new();
        for char in chars.chars() {
            if char.is_ascii() {
                ascii[char as usize] = true;
            } else {
                other.insert(char);
            }
        }
        Self { ascii, other }
    }

    /// Builds the rule from its arguments: `{"leave_only": " -"}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the characters are missing.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(&string_arg(args, 0, "leave_only")?))
    }

    #[inline]
    fn is_listed(&self, char: char) -> bool {
        if char.is_ascii() {
            self.ascii[char as usize]
        } else {
            self.other.contains(&char)
        }
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) || !is_primitive(value) {
            return Outcome::Valid;
        }
        let str = to_js_string(value);
        if value.is_string() && str.chars().all(|char| self.is_listed(char)) {
            return Outcome::Valid;
        }
        Outcome::Value(Value::String(
            str.chars().filter(|&char| self.is_listed(char)).collect(),
        ))
    }
}
