use std::sync::LazyLock;

use regex::Regex;

use crate::rules::prelude::*;

/// `url`: the value must be an http or https URL.
#[derive(Clone, Copy, Debug, Default)]
pub struct Url;

// The regular expression of the JavaScript implementation. `\S` and `\d`
// are written out as the classes of JavaScript; characters from U+00A1 up
// include those outside of the BMP, which JavaScript sees as two code units
// in the range
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    const NOT_SPACE: &str = r"[^\t\n\x0B\f\r \x{a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}\x{feff}]";
    const ALNUM: &str = r"[a-z\x{a1}-\x{10ffff}0-9]";
    const ALPHA: &str = r"[a-z\x{a1}-\x{10ffff}]";
    let pattern = format!(
        "(?i)^(?:(?:http|https)://)(?:{s}+(?::{s}*)?@)?(?:\
         (?:(?:[1-9][0-9]?|1[0-9][0-9]|2[0-1][0-9]|22[0-3])(?:\\.(?:1?[0-9]{{1,2}}|2[0-4][0-9]|25[0-5])){{2}}(?:\\.(?:[0-9][0-9]?|1[0-9][0-9]|2[0-4][0-9]|25[0-4]))\
         |(?:(?:{a}-*)*{a}+)(?:\\.(?:{a}-*)*{a}+)*(?:\\.(?:{l}{{2,}}))\
         )\\.?|localhost)(?::[0-9]{{2,5}})?(?:[/?#]{s}*)?$",
        s = NOT_SPACE,
        a = ALNUM,
        l = ALPHA,
    );
    Regex::new(&pattern).expect("valid URL regular expression")
});

// Longer URLs do not work in some browsers
const MAX_URL_LENGTH: usize = 2083;

impl Url {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// None, the signature is the one of all rule builders.
    pub fn build(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) {
            return Outcome::Valid;
        }
        if !is_primitive(value) {
            return Outcome::code("FORMAT_ERROR");
        }
        let str = to_js_string(value);
        // The length in UTF-16 code units, like in JavaScript
        let is_short = str.len() < MAX_URL_LENGTH || str.encode_utf16().count() < MAX_URL_LENGTH;
        if is_short && URL_RE.is_match(&str) {
            Outcome::Valid
        } else {
            Outcome::code("WRONG_URL")
        }
    }
}
