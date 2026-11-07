use crate::rules::prelude::*;

/// `hex` (LIVR 2.5): hex digits `[0-9a-fA-F]+`, no `0x` prefix.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hex;

impl Hex {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "hex")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_HEX", |bytes| {
            bytes.iter().all(|&byte| is_hex_digit(byte))
        })
    }
}
