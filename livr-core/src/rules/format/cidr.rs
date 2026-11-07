use crate::rules::prelude::*;

/// `cidr` (LIVR 2.5): an `ip` address, `/` and a prefix length, `0`-`32` for
/// IPv4, `0`-`128` for IPv6, without leading zeros. Host bits may be set.
/// `{"cidr": "v4"}` or `{"cidr": "v6"}` for one of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cidr {
    // 0 for both
    version: u8,
}

impl Cidr {
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
                    "rule [cidr] takes no arguments, \"v4\" or \"v6\"",
                ));
            }
        };
        Ok(Self { version })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_CIDR", |bytes| self.is_cidr(bytes))
    }

    fn is_cidr(self, bytes: &[u8]) -> bool {
        // An address never has `/`
        let Some(slash) = bytes.iter().position(|&byte| byte == b'/') else {
            return false;
        };
        let (address, prefix) = (&bytes[..slash], &bytes[slash + 1..]);
        let max_prefix = if self.version != 6 && is_ipv4(address) {
            32
        } else if self.version != 4 && is_ipv6(address) {
            128
        } else {
            return false;
        };
        let is_decimal = matches!(prefix.len(), 1..=3)
            && prefix.iter().all(u8::is_ascii_digit)
            && (prefix.len() == 1 || prefix[0] != b'0');
        is_decimal
            && prefix.iter().fold(0_u32, |number, &digit| {
                number * 10 + u32::from(digit - b'0')
            }) <= max_prefix
    }
}
