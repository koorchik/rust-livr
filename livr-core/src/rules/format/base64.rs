use crate::rules::prelude::*;

/// `base64` (LIVR 2.5): RFC 4648, the standard alphabet with the padding;
/// `{"base64": "url"}` for the URL-safe alphabet, the padding optional. No
/// whitespace; the unused bits of the last character are not checked.
#[derive(Clone, Copy, Debug, Default)]
pub struct Base64 {
    is_url: bool,
}

impl Base64 {
    /// Builds the rule from its arguments: none or `"url"`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        match args {
            [] => Ok(Self { is_url: false }),
            // Arguments after the variant are ignored
            [variant, ..] if variant == "url" => Ok(Self { is_url: true }),
            _ => Err(SchemaError::new(
                "rule [base64] takes no arguments or \"url\"",
            )),
        }
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_BASE64", |bytes| self.is_base64(bytes))
    }

    fn is_base64(self, bytes: &[u8]) -> bool {
        let padding = bytes.iter().rev().take_while(|&&byte| byte == b'=').count();
        let data = &bytes[..bytes.len() - padding];
        let is_alphabet = |&byte: &u8| {
            byte.is_ascii_alphanumeric()
                || if self.is_url {
                    byte == b'-' || byte == b'_'
                } else {
                    byte == b'+' || byte == b'/'
                }
        };
        if padding > 2 || !data.iter().all(is_alphabet) {
            return false;
        }
        // Without padding, a URL-safe length is never 1 modulo 4: no bytes
        // give it
        if self.is_url && padding == 0 {
            bytes.len() % 4 != 1
        } else {
            bytes.len().is_multiple_of(4)
        }
    }
}
