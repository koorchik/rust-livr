use std::sync::LazyLock;

use regex::Regex;

use crate::rules::prelude::*;

/// `url`: the value must be an http or https URL.
#[derive(Clone, Copy, Debug, Default)]
pub struct Url;

// The regular expression of the JavaScript implementation. `\S` and `\d`
// are written out as the classes of JavaScript; characters from U+00A1 up
// include those outside of the BMP, which JavaScript sees as two code units
// in the range. LIVR 2.5: labels of 1-63 characters (code points here), the
// top-level domain included, nothing but a port, a path, a query or a
// fragment after the host (no dot), ports 0-65535 of 1-5 digits
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    const NOT_SPACE: &str = r"[^\t\n\x0B\f\r \x{a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}\x{feff}]";
    const ALNUM: &str = r"[a-z\x{a1}-\x{10ffff}0-9]";
    const ALNUM_OR_HYPHEN: &str = r"[a-z\x{a1}-\x{10ffff}0-9-]";
    const ALPHA: &str = r"[a-z\x{a1}-\x{10ffff}]";
    const PORT: &str =
        "(?:[0-9]{1,4}|[0-5][0-9]{4}|6[0-4][0-9]{3}|65[0-4][0-9]{2}|655[0-2][0-9]|6553[0-5])";
    // A label does not start or end with "-" and does not contain "."
    let pattern = format!(
        "(?i)^(?:(?:http|https)://)(?:{s}+(?::{s}*)?@)?(?:\
         (?:[1-9][0-9]?|1[0-9][0-9]|2[0-1][0-9]|22[0-3])(?:\\.(?:1?[0-9]{{1,2}}|2[0-4][0-9]|25[0-5])){{2}}(?:\\.(?:[0-9][0-9]?|1[0-9][0-9]|2[0-4][0-9]|25[0-4]))\
         |(?:{a}(?:{h}{{0,61}}{a})?\\.)+{l}{{2,63}}\
         |localhost)(?::{p})?(?:[/?#]{s}*)?$",
        s = NOT_SPACE,
        a = ALNUM,
        h = ALNUM_OR_HYPHEN,
        l = ALPHA,
        p = PORT,
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn is_url(url: &str) -> bool {
        !Url.check(&Value::from(url), &Ctx::none()).is_error()
    }

    #[test]
    fn rejects_a_dot_after_the_host() {
        for url in [
            "https://example.com.",
            "https://www.google.",
            "https://example.com./path",
            "https://example.com.:8080",
            "https://example.com.?q=1",
            "https://example.com.#top",
            "http://192.0.2.1./",
        ] {
            assert!(!is_url(url), "{url}");
        }
        for url in [
            "https://example.com/file.",
            "https://example.com/?q=a.",
            "https://example.com/#a.",
        ] {
            assert!(is_url(url), "{url}");
        }
    }

    #[test]
    fn takes_host_labels_of_up_to_63_characters() {
        for url in [
            format!("https://{}.example.com/", "a".repeat(63)),
            format!("https://example.{}", "a".repeat(63)),
            format!("https://a{}a.com", "-".repeat(61)),
            format!("https://{}.рф", "я".repeat(63)),
        ] {
            assert!(is_url(&url), "{url}");
        }
        for url in [
            format!("https://{}.example.com/", "a".repeat(64)),
            format!("https://example.{}", "a".repeat(64)),
            format!("https://a{}a.com", "-".repeat(62)),
            format!("https://{}.рф", "я".repeat(64)),
        ] {
            assert!(!is_url(&url), "{url}");
        }
    }

    #[test]
    fn takes_ports_from_0_to_65535() {
        for url in [
            "https://example.com:0",
            "https://example.com:8",
            "https://example.com:80/path",
            "https://example.com:00080",
            "https://example.com:65535/",
            "http://example.com:8/path?q=1#top",
            "http://192.0.2.1:8/",
            "http://localhost:8",
        ] {
            assert!(is_url(url), "{url}");
        }
        for url in [
            "https://example.com:65536",
            "https://example.com:99999/",
            "https://example.com:123456",
            "https://example.com:",
            "https://example.com:8a",
            "https://example.com:\u{ff18}",
        ] {
            assert!(!is_url(url), "{url}");
        }
    }

    #[test]
    fn keeps_the_rest_of_the_grammar() {
        for url in [
            "https://пример.рф",
            "https://münchen.de/straße",
            "https://xn--mnchen-3ya.de",
            "https://a--b.example.com",
            "http://localhost",
            "http://LOCALHOST:3000/x",
            "http://10.0.0.1/",
            "HTTP://127.0.0.1:3233/?param_1=123&param_2=asd",
            "https://user:pass@example.com",
        ] {
            assert!(is_url(url), "{url}");
        }
        for url in [
            "http://intranet/",
            "http://0.0.0.0/",
            "http://1.2.3.255/",
            "http://224.0.0.1/",
            "http://256.1.1.1/",
            "https://example.c",
            "https://example.123",
            "https://example..com",
            "https://-a.example.com",
            "https://a-.example.com",
            "http://g_oogle.com",
            "https://example.com/a b",
            "ftp://example.com",
        ] {
            assert!(!is_url(url), "{url}");
        }
    }

    #[test]
    fn rejects_long_invalid_values_quickly() {
        let urls = [
            format!("http://{}_", "aaaa.".repeat(12)),
            format!("http://{}_", "aaaa-".repeat(12)),
            format!("http://{}a!", "aaaa.".repeat(12)),
            format!("http://user@{}_", "aaaa.".repeat(12)),
            format!("http://{}{}_", "a@".repeat(12), "aaaa.".repeat(12)),
            format!("http://{}_", "aaaa.".repeat(400)),
            format!("http://{}!", "a".repeat(2000)),
            format!("http://{}_", "a@".repeat(1000)),
            format!("http://{} ", "a@a.com/".repeat(250)),
            format!("http://{}_", "a:".repeat(1000)),
            format!("http://{}_", "я.".repeat(1000)),
        ];
        // Builds the regular expression
        assert!(is_url("https://example.com"));

        let start = Instant::now();
        for url in &urls {
            assert!(!is_url(url), "{url}");
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 50, "{elapsed:?}");
    }
}
