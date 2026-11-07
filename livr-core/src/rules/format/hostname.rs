use crate::rules::prelude::*;

/// `hostname` (LIVR 2.5): RFC 1123 labels of 1-63 characters `A-Za-z0-9-`,
/// not starting or ending with `-`, separated by `.`, at most 253 characters,
/// no trailing dot. The last label is not all digits, so an IPv4 address is
/// not a hostname.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hostname;

impl Hostname {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "hostname")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_HOSTNAME", is_hostname)
    }
}

fn is_hostname(bytes: &[u8]) -> bool {
    if bytes.len() > 253 {
        return false;
    }
    let mut is_last_all_digits = false;
    for label in bytes.split(|&byte| byte == b'.') {
        let is_valid = matches!(label.len(), 1..=63)
            && label[0] != b'-'
            && label[label.len() - 1] != b'-'
            && label
                .iter()
                .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'-');
        if !is_valid {
            return false;
        }
        is_last_all_digits = label.iter().all(u8::is_ascii_digit);
    }
    !is_last_all_digits
}
