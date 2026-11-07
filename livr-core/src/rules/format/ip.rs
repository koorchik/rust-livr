use crate::rules::prelude::*;

/// `ip` (LIVR 2.5): an IPv4 address without leading zeros, or an IPv6
/// address in the text forms of RFC 4291; `{"ip": "v4"}` or `{"ip": "v6"}`
/// for one of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ip {
    // 0 for both
    version: u8,
}

impl Ip {
    /// Builds the rule from its arguments: none, `"v4"` or `"v6"`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        // Arguments after the version are ignored
        let version = match args {
            [] => 0,
            [version, ..] if version == "v4" => 4,
            [version, ..] if version == "v6" => 6,
            _ => {
                return Err(SchemaError::new(
                    "rule [ip] takes no arguments, \"v4\" or \"v6\"",
                ));
            }
        };
        Ok(Self { version })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_IP", |bytes| {
            (self.version != 6 && is_ipv4(bytes)) || (self.version != 4 && is_ipv6(bytes))
        })
    }
}
