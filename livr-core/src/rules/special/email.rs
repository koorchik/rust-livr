use crate::rules::prelude::*;

/// `email`: the value must be an email address.
#[derive(Clone, Copy, Debug, Default)]
pub struct Email;

impl Email {
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
        if is_email(&to_js_string(value)) {
            Outcome::Valid
        } else {
            Outcome::code("WRONG_EMAIL")
        }
    }
}

// A scanner for the regular expression of the JavaScript implementation:
//
//   ^(?:[^<>()\[\]\\.,;:\s@"]+(?:\.[^<>()\[\]\\.,;:\s@"]+)*|"[^@\n\r  ]+")
//   @(?:\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\]|(?:[a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,})$
//
// Neither the local part nor the domain can contain "@", so the address has
// exactly one, and both parts are checked without backtracking
fn is_email(str: &str) -> bool {
    let Some(at) = str.find('@') else {
        return false;
    };
    let (local, domain) = (&str[..at], &str[at + 1..]);
    !local.is_empty() && !domain.contains('@') && is_local_part(local) && is_domain(domain)
}

// Atoms separated by dots, or a quoted string
fn is_local_part(local: &str) -> bool {
    if let Some(quoted) = local.strip_prefix('"') {
        // At least one character between the quotes
        return match quoted.strip_suffix('"') {
            Some(inner) if !inner.is_empty() => {
                !inner.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
            }
            _ => false,
        };
    }
    // One pass over the bytes: "." between non-empty atoms
    let mut atom_length = 0;
    if local.is_ascii() {
        for byte in local.bytes() {
            if byte == b'.' {
                if atom_length == 0 {
                    return false;
                }
                atom_length = 0;
            } else if ASCII_SPECIAL_IN_ATOM[usize::from(byte)] {
                return false;
            } else {
                atom_length += 1;
            }
        }
        return atom_length > 0;
    }
    for char in local.chars() {
        if char == '.' {
            if atom_length == 0 {
                return false;
            }
            atom_length = 0;
        } else if is_special_in_atom(char) {
            return false;
        } else {
            atom_length += 1;
        }
    }
    atom_length > 0
}

// The ASCII characters of is_special_in_atom, as a table
static ASCII_SPECIAL_IN_ATOM: [bool; 128] = {
    let mut table = [false; 128];
    let specials = b"\t\n\x0B\x0C\r <>()[]\\,;:\"";
    let mut i = 0;
    while i < specials.len() {
        table[specials[i] as usize] = true;
        i += 1;
    }
    table
};

// <>()[]\,;:" and whitespace (\s of JavaScript regular expressions)
fn is_special_in_atom(char: char) -> bool {
    matches!(
        char,
        '\t'..='\r'
            | ' '
            | '<'
            | '>'
            | '('
            | ')'
            | '['
            | ']'
            | '\\'
            | ','
            | ';'
            | ':'
            | '"'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

// An IPv4 address in brackets, or two or more labels of letters, digits and
// "-", where the last one has 2+ letters only
fn is_domain(domain: &str) -> bool {
    if let Some(ip) = domain.strip_prefix('[') {
        return ip.strip_suffix(']').is_some_and(is_ip_literal);
    }
    // One pass: labels of letters, digits and "-", the last one of 2+
    // letters, at least two labels
    let mut label_length = 0;
    let mut labels = 0;
    let mut is_letter_label = true;
    for byte in domain.bytes() {
        if byte == b'.' {
            if label_length == 0 {
                return false;
            }
            labels += 1;
            label_length = 0;
            is_letter_label = true;
        } else if byte.is_ascii_alphabetic() {
            label_length += 1;
        } else if byte.is_ascii_digit() || byte == b'-' {
            label_length += 1;
            is_letter_label = false;
        } else {
            return false;
        }
    }
    labels >= 1 && label_length >= 2 && is_letter_label
}

// Four groups of 1-3 digits
fn is_ip_literal(ip: &str) -> bool {
    let mut groups = 0;
    for group in ip.split('.') {
        groups += 1;
        if group.is_empty() || group.len() > 3 || !group.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    groups == 4
}

#[cfg(test)]
mod tests {
    use super::is_email;

    #[test]
    fn checks_emails_like_javascript() {
        for email in [
            "a@b.cc",
            "a.b@c.d.finance",
            "!#$%&*+/=?^`|@x.org",
            "\"a b\"@x.org",
            "a@[1.2.3.4]",
            "a@1-2.com",
        ] {
            assert!(is_email(email), "{email}");
        }
        for email in [
            "",
            "@x.org",
            "a@",
            "a@@x.org",
            "a@b@x.org",
            "a@x",
            "a@x.c",
            "a@x.c1",
            "a..b@x.org",
            ".a@x.org",
            "a.@x.org",
            "a b@x.org",
            "a@x_y.org",
            "a(b)@x.org",
            "a@x.org\n",
            "a@.x.org",
            "a@x..org",
            "\"\"@x.org",
            "a@[1.2.3]",
            "a@[1.2.3.4444]",
            "a@[1.2.3.4",
            "a\u{a0}b@x.org",
        ] {
            assert!(!is_email(email), "{email:?}");
        }
    }
}
