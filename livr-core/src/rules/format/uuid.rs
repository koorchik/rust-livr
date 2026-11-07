use crate::rules::prelude::*;

/// `uuid` (LIVR 2.5): `HEX{8}-HEX{4}-HEX{4}-HEX{4}-HEX{12}` (RFC 9562) with a
/// version 1-8 and the variant `8`, `9`, `a` or `b`; the nil and the max UUID
/// pass too. `{"uuid": ["v4", "v7"]}` allows only these versions (and not
/// nil or max).
#[derive(Clone, Copy, Debug, Default)]
pub struct Uuid {
    // Allowed version digits as bits, `1 << 4` for v4; 0 for any version
    versions: u16,
}

impl Uuid {
    /// Builds the rule from its arguments: none, or versions `"v1"`-`"v8"`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let mut versions = 0;
        for arg in args {
            match arg.as_str().map(str::as_bytes) {
                Some(&[b'v', digit @ b'1'..=b'8']) => versions |= 1 << (digit - b'0'),
                _ => {
                    return Err(SchemaError::new(format!(
                        "rule [uuid] takes versions \"v1\"-\"v8\", got {arg}"
                    )));
                }
            }
        }
        Ok(Self { versions })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_UUID", |bytes| self.is_uuid(bytes))
    }

    fn is_uuid(self, bytes: &[u8]) -> bool {
        if bytes.len() != 36 {
            return false;
        }
        for (i, &byte) in bytes.iter().enumerate() {
            let is_valid = if matches!(i, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                is_hex_digit(byte)
            };
            if !is_valid {
                return false;
            }
        }
        let version = bytes[14];
        let is_versioned = matches!(version, b'1'..=b'8')
            && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B');
        if self.versions != 0 {
            return is_versioned && self.versions & (1 << (version - b'0')) != 0;
        }
        // The nil and the max UUID have no version and variant
        is_versioned
            || bytes.iter().all(|&byte| byte == b'0' || byte == b'-')
            || bytes.iter().all(|&byte| matches!(byte, b'f' | b'F' | b'-'))
    }
}
