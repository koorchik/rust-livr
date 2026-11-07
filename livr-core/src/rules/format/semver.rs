use crate::rules::prelude::*;

/// `semver` (LIVR 2.5): a version of Semantic Versioning 2.0.0,
/// without a `v` prefix.
#[derive(Clone, Copy, Debug, Default)]
pub struct Semver;

impl Semver {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "semver")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_SEMVER", is_semver)
    }
}

// `0` or a number without leading zeros
fn is_number(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0')
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-'
}

fn is_semver(bytes: &[u8]) -> bool {
    // The build after the first `+`, the pre-release after the first `-` of
    // the rest (the core has no `-`)
    let (rest, build) = match bytes.iter().position(|&byte| byte == b'+') {
        Some(plus) => (&bytes[..plus], Some(&bytes[plus + 1..])),
        None => (bytes, None),
    };
    let (core, pre_release) = match rest.iter().position(|&byte| byte == b'-') {
        Some(minus) => (&rest[..minus], Some(&rest[minus + 1..])),
        None => (rest, None),
    };
    let mut numbers = core.split(|&byte| byte == b'.');
    let is_core = (0..3).all(|_| numbers.next().is_some_and(is_number)) && numbers.next().is_none();
    // A pre-release identifier is a number or has a letter or `-`
    let is_pre_release = pre_release.is_none_or(|pre_release| {
        pre_release.split(|&byte| byte == b'.').all(|id| {
            !id.is_empty()
                && id.iter().all(|&byte| is_identifier_char(byte))
                && (is_number(id) || !id.iter().all(u8::is_ascii_digit))
        })
    });
    let is_build = build.is_none_or(|build| {
        build
            .split(|&byte| byte == b'.')
            .all(|id| !id.is_empty() && id.iter().all(|&byte| is_identifier_char(byte)))
    });
    is_core && is_pre_release && is_build
}
