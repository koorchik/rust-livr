//! Grammars shared by several rules (`ip` and `cidr`), as loops over bytes:
//! cheaper than regular expressions, and ASCII only.

/// Whether the byte is `[0-9a-fA-F]`.
#[inline]
pub(crate) fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

/// An IPv4 address: four numbers 0-255 separated by `.`, without leading
/// zeros (some systems read `010` as octal).
pub(crate) fn is_ipv4(bytes: &[u8]) -> bool {
    let mut i = 0;
    for part in 0..4 {
        if part > 0 {
            if bytes.get(i) != Some(&b'.') {
                return false;
            }
            i += 1;
        }
        let start = i;
        let mut number = 0_u32;
        while i < bytes.len() && i - start < 3 && bytes[i].is_ascii_digit() {
            number = number * 10 + u32::from(bytes[i] - b'0');
            i += 1;
        }
        let length = i - start;
        if length == 0 || number > 255 || (length > 1 && bytes[start] == b'0') {
            return false;
        }
    }
    i == bytes.len()
}

// A group of 1-4 hex digits
fn is_hex_group(bytes: &[u8]) -> bool {
    (1..=4).contains(&bytes.len()) && bytes.iter().all(|&byte| is_hex_digit(byte))
}

// The number of 16-bit groups of hex groups separated by `:`; the last one
// may be an IPv4 address (two groups) at the end of the address. `None` for
// a wrong group
fn count_groups(bytes: &[u8], is_address_end: bool) -> Option<usize> {
    if bytes.is_empty() {
        return Some(0);
    }
    let mut count = 0;
    let mut groups = bytes.split(|&byte| byte == b':').peekable();
    while let Some(group) = groups.next() {
        if is_hex_group(group) {
            count += 1;
        } else if is_address_end && groups.peek().is_none() && is_ipv4(group) {
            count += 2;
        } else {
            return None;
        }
    }
    Some(count)
}

/// An IPv6 address in the text forms of RFC 4291, section 2.2: eight groups,
/// or fewer with one `::` for one or more groups of zeros; the last two
/// groups may be an IPv4 address. No zone, no brackets.
pub(crate) fn is_ipv6(bytes: &[u8]) -> bool {
    let Some(gap) = bytes.windows(2).position(|pair| pair == b"::") else {
        return count_groups(bytes, true) == Some(8);
    };
    // A second `::` (`:::` too) is ambiguous
    if bytes[gap + 1..].windows(2).any(|pair| pair == b"::") {
        return false;
    }
    match (
        count_groups(&bytes[..gap], false),
        count_groups(&bytes[gap + 2..], true),
    ) {
        (Some(before), Some(after)) => before + after <= 7,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_ipv4() {
        for ip in ["0.0.0.0", "192.0.2.1", "255.255.255.255", "10.0.0.10"] {
            assert!(is_ipv4(ip.as_bytes()), "{ip}");
        }
        for ip in [
            "256.0.0.1",
            "010.0.0.1",
            "1.2.3",
            "1.2.3.4.5",
            "1.2.3.4.",
            "1..2.3",
            "1234.1.1.1",
            " 1.2.3.4",
            "1.2.3.4\n",
            "00.1.1.1",
        ] {
            assert!(!is_ipv4(ip.as_bytes()), "{ip:?}");
        }
    }

    #[test]
    fn checks_ipv6() {
        for ip in [
            "::",
            "::1",
            "1::",
            "2001:db8::8a2e:370:7334",
            "2001:0db8:0000:0000:0000:ff00:0042:8329",
            "::ffff:192.0.2.1",
            "1:2:3:4:5:6:192.0.2.1",
            "1:2:3:4:5:6:7::",
            "::2:3:4:5:6:7:8",
            "1:2:3:4:5::192.0.2.1",
        ] {
            assert!(is_ipv6(ip.as_bytes()), "{ip}");
        }
        for ip in [
            ":",
            ":::",
            "1:::2",
            "1::2::3",
            ":1::",
            "1::2:",
            "1:2:3:4:5:6:7",
            "1:2:3:4:5:6:7:8:9",
            "1:2:3:4:5:6:7::8",
            "12345::",
            "g::",
            "1:2:3:4:5:6:7:192.0.2.1",
            "192.0.2.1::",
            "fe80::1%eth0",
            "[::1]",
            "::1/128",
        ] {
            assert!(!is_ipv6(ip.as_bytes()), "{ip:?}");
        }
    }
}
