use crate::rules::prelude::*;

/// `mac` (LIVR 2.5): six pairs of hex digits (MAC-48, EUI-48) separated by
/// `:` or by `-`, the same separator everywhere; `{"mac": 64}` for eight
/// pairs (EUI-64).
#[derive(Clone, Copy, Debug)]
pub struct Mac {
    pairs: usize,
}

impl Default for Mac {
    fn default() -> Self {
        Self { pairs: 6 }
    }
}

impl Mac {
    /// Builds the rule from its arguments: none, `48` or `64` (also as
    /// strings).
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        // A number or a string with one; arguments after it are ignored
        let bits = match args {
            [] => Some(48),
            [bits, ..] => count_arg(bits, "mac").ok(),
        };
        let pairs = match bits {
            Some(48) => 6,
            Some(64) => 8,
            _ => {
                return Err(SchemaError::new("rule [mac] takes no arguments, 48 or 64"));
            }
        };
        Ok(Self { pairs })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_MAC", |bytes| {
            if bytes.len() != self.pairs * 3 - 1 {
                return false;
            }
            let separator = bytes[2];
            (separator == b':' || separator == b'-')
                && bytes.iter().enumerate().all(|(i, &byte)| {
                    if i % 3 == 2 {
                        byte == separator
                    } else {
                        is_hex_digit(byte)
                    }
                })
        })
    }
}
