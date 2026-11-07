use crate::rules::prelude::*;

/// `e164` (LIVR 2.5): a phone number in the E.164 format, `+`
/// and 7-15 digits, the first is not `0`.
#[derive(Clone, Copy, Debug, Default)]
pub struct E164;

impl E164 {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "e164")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_E164", is_e164)
    }
}

fn is_e164(bytes: &[u8]) -> bool {
    match bytes {
        [b'+', b'1'..=b'9', rest @ ..] => {
            (6..=14).contains(&rest.len()) && rest.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}
