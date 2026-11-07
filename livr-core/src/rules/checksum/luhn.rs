use crate::rules::prelude::*;

/// `luhn` (LIVR 2.5): at least 2 ASCII digits with a valid Luhn mod 10
/// checksum, checked as they are (no spaces or dashes are removed).
#[derive(Clone, Copy, Debug, Default)]
pub struct Luhn;

impl Luhn {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "luhn")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_CHECKSUM", |bytes| {
            bytes.len() >= 2 && has_luhn_checksum(bytes)
        })
    }
}

// Every second digit from the right is doubled (minus 9 above 9)
fn has_luhn_checksum(bytes: &[u8]) -> bool {
    let mut sum = 0;
    for (i, &byte) in bytes.iter().rev().enumerate() {
        if !byte.is_ascii_digit() {
            return false;
        }
        let mut digit = u32::from(byte - b'0');
        if i % 2 == 1 {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
    }
    sum % 10 == 0
}
