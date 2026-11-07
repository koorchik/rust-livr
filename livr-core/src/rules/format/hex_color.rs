use crate::rules::prelude::*;

/// `hex_color` (LIVR 2.5): `#` and 3, 4, 6 or 8 hex digits.
#[derive(Clone, Copy, Debug, Default)]
pub struct HexColor;

impl HexColor {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "hex_color")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_HEX_COLOR", is_hex_color)
    }
}

fn is_hex_color(bytes: &[u8]) -> bool {
    match bytes.split_first() {
        Some((b'#', digits)) => {
            matches!(digits.len(), 3 | 4 | 6 | 8) && digits.iter().all(|&byte| is_hex_digit(byte))
        }
        _ => false,
    }
}
