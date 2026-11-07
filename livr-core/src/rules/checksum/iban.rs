use crate::rules::prelude::*;

/// `iban` (LIVR 2.5): ISO 13616, 2 uppercase letters, 2 digits and 11-30
/// characters `A-Z0-9`, with a valid mod 97 checksum. Checked as it is: no
/// spaces are removed, lowercase letters fail. No per-country lengths.
#[derive(Clone, Copy, Debug, Default)]
pub struct Iban;

impl Iban {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "iban")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_IBAN", is_iban)
    }
}

fn is_iban(bytes: &[u8]) -> bool {
    let is_format = (15..=34).contains(&bytes.len())
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..4].iter().all(u8::is_ascii_digit)
        && bytes[4..]
            .iter()
            .all(|&byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
    if !is_format {
        return false;
    }
    // The first four characters moved to the end, a letter is two digits
    // (A = 10 ... Z = 35); the remainder a character at a time stays small
    let remainder = bytes[4..]
        .iter()
        .chain(&bytes[..4])
        .fold(0_u32, |remainder, &byte| {
            if byte.is_ascii_digit() {
                (remainder * 10 + u32::from(byte - b'0')) % 97
            } else {
                (remainder * 100 + u32::from(byte - b'A') + 10) % 97
            }
        });
    remainder == 1
}
