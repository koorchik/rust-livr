use crate::rules::prelude::*;

/// `credit_card` (LIVR 2.5): 12-19 ASCII digits (ISO/IEC 7812) with a valid
/// Luhn checksum. No card network detection.
#[derive(Clone, Copy, Debug, Default)]
pub struct CreditCard;

impl CreditCard {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "credit_card")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_CREDIT_CARD_NUMBER", |bytes| {
            (12..=19).contains(&bytes.len()) && has_luhn_checksum(bytes)
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
