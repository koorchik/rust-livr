use crate::rules::prelude::*;

/// `digest` (LIVR 2.5): the hex digest of one of the algorithms, by its
/// length: `{"digest": "sha256"}`, `{"digest": ["md5", "sha1"]}`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Digest {
    // Allowed lengths in hex digits as bits: `1 << (32 / 8)` for md5
    lengths: u32,
}

impl Digest {
    /// Builds the rule from its arguments: `"md5"`, `"sha1"`, `"sha256"`,
    /// `"sha384"` or `"sha512"`, one or more.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without algorithms or for an unknown one.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        if args.is_empty() {
            return Err(SchemaError::new(
                "rule [digest] needs one or more algorithms",
            ));
        }
        let mut lengths = 0;
        for arg in args {
            let length = match arg.as_str() {
                Some("md5") => 32,
                Some("sha1") => 40,
                Some("sha256") => 64,
                Some("sha384") => 96,
                Some("sha512") => 128,
                _ => {
                    return Err(SchemaError::new(format!(
                        "rule [digest] got an unknown algorithm {arg}"
                    )));
                }
            };
            lengths |= 1 << (length / 8);
        }
        Ok(Self { lengths })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_DIGEST", |bytes| {
            let length = bytes.len();
            length % 8 == 0
                && length <= 128
                && self.lengths & (1 << (length / 8)) != 0
                && bytes.iter().all(|&byte| is_hex_digit(byte))
        })
    }
}
