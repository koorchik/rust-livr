use crate::rules::prelude::*;

/// `jwt` (LIVR 2.5): the structure of a JWS compact serialization:
/// three base64url segments without padding separated by `.`, the signature
/// may be empty. Nothing is decoded or verified.
#[derive(Clone, Copy, Debug, Default)]
pub struct Jwt;

impl Jwt {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "jwt")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_JWT", is_jwt)
    }
}

fn is_jwt(bytes: &[u8]) -> bool {
    let mut segments = bytes.split(|&byte| byte == b'.');
    let (Some(header), Some(payload), Some(signature), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };
    // No bytes give a base64url length of 1 modulo 4
    let is_base64url = |segment: &[u8]| {
        segment.len() % 4 != 1
            && segment
                .iter()
                .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    };
    !header.is_empty()
        && !payload.is_empty()
        && [header, payload, signature].into_iter().all(is_base64url)
}
