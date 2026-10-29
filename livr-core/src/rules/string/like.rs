use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;

use crate::rules::prelude::*;

/// `like`: the value must match a regular expression (`{"like": "^[a-z]+$"}`,
/// or `{"like": ["^[a-z]+$", "i"]}` to ignore case). The expression is in
/// the JavaScript syntax without the `u` flag, and matches like it: `\d`,
/// `\w` and `\b` are ASCII only, `.` stops at line terminators, and with `i`
/// ASCII letters match only ASCII letters. Look-around, backreferences and
/// inline flags are schema errors.
#[derive(Clone, Debug)]
pub struct Like {
    // Shared: a clone of a `Regex` gets its own cache of matching states,
    // allocated again on its first match
    re: Arc<Regex>,
}

// Compiled regular expressions by pattern and case flag. A `Regex` is cheap
// to clone, so validators built for every validation share them instead of
// compiling the same pattern again
type Cache = HashMap<(String, bool), Arc<Regex>>;

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

// Patterns built from data must not fill the memory
const MAX_CACHE_SIZE: usize = 256;

impl Like {
    /// The rule for a pattern.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid or unsupported pattern.
    pub fn new(pattern: &str, ignore_case: bool) -> Result<Self, SchemaError> {
        let key = (pattern.to_owned(), ignore_case);
        if let Some(re) = CACHE.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
            return Ok(Self { re });
        }
        let invalid = |error: &dyn std::fmt::Display| {
            SchemaError::new(format!(
                "rule [like] got an invalid regular expression [{pattern}]: {error}"
            ))
        };
        // The translation folds the case itself, the way JavaScript does
        let translated = translate(pattern, ignore_case).map_err(|error| invalid(&error))?;
        let re = Regex::new(&translated)
            .map(Arc::new)
            .map_err(|error| invalid(&error))?;
        if let Ok(mut cache) = CACHE.lock() {
            if cache.len() >= MAX_CACHE_SIZE {
                cache.clear();
            }
            cache.insert(key, re.clone());
        }
        Ok(Self { re })
    }

    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for a missing, invalid or unsupported pattern.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let pattern = string_arg(args, 0, "like")?;
        let ignore_case = matches!(args.get(1), Some(Value::String(flags)) if flags.contains('i'));
        Self::new(&pattern, ignore_case)
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
        if !self.re.is_match(&str) {
            return Outcome::code("WRONG_FORMAT");
        }
        string_output(value, str)
    }
}

// A JavaScript regular expression (without the `u` flag) in the syntax of
// the `regex` crate, with the same meaning. Classes are written out as
// ranges of code points, so the special characters of Rust classes (`[`,
// `&&`, `--`, `~~`) need no care. With `ignore_case`, the case of letters is
// folded here and not by `regex`: `regex` folds with Unicode rules, where
// `k` matches the Kelvin sign and `s` the long s, which JavaScript keeps
// apart.
//
// Characters beyond U+FFFF stay whole characters, where JavaScript sees two
// UTF-16 units: `.` and classes match them whole.
fn translate(pattern: &str, ignore_case: bool) -> Result<String, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut i = 0;
    while let Some(&char) = chars.get(i) {
        i += 1;
        match char {
            '\\' => {
                let Some(&escaped) = chars.get(i) else {
                    return Err("\\ at the end of the pattern".to_owned());
                };
                match escaped {
                    'd' | 'D' | 'w' | 'W' | 's' | 'S' => {
                        i += 1;
                        let (ranges, is_negated) = class_escape(escaped);
                        push_class(&mut out, ranges.to_vec(), is_negated, ignore_case);
                    }
                    // Boundaries of ASCII words
                    'b' => {
                        i += 1;
                        out.push_str(r"(?-u:\b)");
                    }
                    'B' => {
                        i += 1;
                        out.push_str(r"(?-u:\B)");
                    }
                    'k' if chars.get(i + 1) == Some(&'<') => {
                        return Err("backreferences are not supported".to_owned());
                    }
                    _ => {
                        let (code, length) = char_escape(&chars[i..], false)?;
                        i += length;
                        push_char(&mut out, code, ignore_case)?;
                    }
                }
            }
            '.' => out.push_str(r"[^\n\r\x{2028}\x{2029}]"),
            '[' => i = class(&chars, i, &mut out, ignore_case)?,
            // A brace that does not start a quantifier is a literal
            '{' => match quantifier_length(&chars[i..]) {
                Some(length) => {
                    out.push('{');
                    out.extend(&chars[i..i + length]);
                    i += length;
                }
                None => out.push_str(r"\{"),
            },
            '}' | ']' => {
                out.push('\\');
                out.push(char);
            }
            '(' => {
                out.push('(');
                if chars.get(i) == Some(&'?') {
                    i = group_prefix(&chars, i, &mut out)?;
                }
            }
            '^' | '$' | '|' | ')' | '*' | '+' | '?' => out.push(char),
            char => push_char(&mut out, u32::from(char), ignore_case)?,
        }
    }
    Ok(out)
}

// From `?` after `(`: a non-capturing or a named group; returns the index
// after it. Look-around and inline flags have no equivalent: an error rather
// than another meaning
fn group_prefix(chars: &[char], mut i: usize, out: &mut String) -> Result<usize, String> {
    match (chars.get(i + 1), chars.get(i + 2)) {
        (Some(':'), _) => {
            out.push_str("?:");
            Ok(i + 2)
        }
        (Some('<'), Some(next)) if !matches!(next, '=' | '!') => {
            out.push_str("?<");
            i += 2;
            // The name is copied as it is, up to `>`
            while let Some(&char) = chars.get(i) {
                i += 1;
                out.push(char);
                if char == '>' {
                    return Ok(i);
                }
            }
            Err("unterminated group name".to_owned())
        }
        (Some('=' | '!' | '<'), _) => Err("look-around is not supported".to_owned()),
        _ => Err("inline flags are not supported".to_owned()),
    }
}

// The length of a quantifier after its `{`, up to its `}`: `{n}`, `{n,}` or
// `{n,m}`
fn quantifier_length(chars: &[char]) -> Option<usize> {
    let digits = |from: usize| {
        chars[from.min(chars.len())..]
            .iter()
            .take_while(|char| char.is_ascii_digit())
            .count()
    };
    let min = digits(0);
    if min == 0 {
        return None;
    }
    let mut i = min;
    if chars.get(i) == Some(&',') {
        i += 1;
        i += digits(i);
    }
    (chars.get(i) == Some(&'}')).then_some(i + 1)
}

// The code point of an escape that stands for one character, from the
// character after `\`, and the length of the escape without the `\`.
// Letters and signs without a meaning stand for themselves, as in JavaScript
// without the `u` flag (`\z` is `z`)
fn char_escape(chars: &[char], is_in_class: bool) -> Result<(u32, usize), String> {
    let hex = |length: usize| {
        let digits: String = chars.get(1..=length)?.iter().collect();
        if digits.chars().all(|char| char.is_ascii_hexdigit()) {
            u32::from_str_radix(&digits, 16).ok()
        } else {
            None
        }
    };
    let code = match chars[0] {
        'n' => u32::from('\n'),
        'r' => u32::from('\r'),
        't' => u32::from('\t'),
        'v' => 0x0b,
        'f' => 0x0c,
        // A backspace in a class (a word boundary outside)
        'b' if is_in_class => 0x08,
        '0' if !chars.get(1).is_some_and(char::is_ascii_digit) => 0,
        '0'..='9' => {
            return Err("backreferences and octal escapes are not supported".to_owned());
        }
        // A control character; in a class, digits and `_` too
        'c' => match chars.get(1) {
            Some(&letter)
                if letter.is_ascii_alphabetic()
                    || (is_in_class && (letter.is_ascii_digit() || letter == '_')) =>
            {
                return Ok((u32::from(letter) % 32, 2));
            }
            // A backslash, and `c` as it is after it
            _ => return Ok((u32::from('\\'), 0)),
        },
        'x' => match hex(2) {
            Some(code) => return Ok((code, 3)),
            None => u32::from('x'),
        },
        'u' => match hex(4) {
            // A surrogate pair is one character in UTF-8
            Some(high @ 0xd800..=0xdbff) if !is_in_class => {
                let low = match chars.get(5..11) {
                    Some(['\\', 'u', digits @ ..]) => {
                        let digits: String = digits.iter().collect();
                        u32::from_str_radix(&digits, 16)
                            .ok()
                            .filter(|low| (0xdc00..=0xdfff).contains(low))
                    }
                    _ => None,
                };
                let low = low.ok_or("lone surrogates are not supported")?;
                return Ok((0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00), 11));
            }
            Some(0xd800..=0xdfff) => return Err("surrogates are not supported".to_owned()),
            Some(code) => return Ok((code, 5)),
            None => u32::from('u'),
        },
        escaped => u32::from(escaped),
    };
    Ok((code, 1))
}

// The ranges of `\d`, `\w` and `\s`, and whether the escape is their
// complement (`\D`, `\W`, `\S`). `\s` is the whitespace of JavaScript:
// Unicode spaces and line terminators and U+FEFF, but not U+0085
fn class_escape(escape: char) -> (&'static [(u32, u32)], bool) {
    const DIGITS: &[(u32, u32)] = &[(0x30, 0x39)];
    const WORD: &[(u32, u32)] = &[(0x30, 0x39), (0x41, 0x5a), (0x5f, 0x5f), (0x61, 0x7a)];
    const SPACE: &[(u32, u32)] = &[
        (0x09, 0x0d),
        (0x20, 0x20),
        (0xa0, 0xa0),
        (0x1680, 0x1680),
        (0x2000, 0x200a),
        (0x2028, 0x2029),
        (0x202f, 0x202f),
        (0x205f, 0x205f),
        (0x3000, 0x3000),
        (0xfeff, 0xfeff),
    ];
    let ranges = match escape.to_ascii_lowercase() {
        'd' => DIGITS,
        'w' => WORD,
        _ => SPACE,
    };
    (ranges, escape.is_ascii_uppercase())
}

// An item of a class: a character, or the ranges of a class escape
enum ClassItem {
    Char(u32),
    Ranges(&'static [(u32, u32)], bool),
}

fn add_item(ranges: &mut Vec<(u32, u32)>, item: ClassItem) {
    match item {
        ClassItem::Char(code) => ranges.push((code, code)),
        ClassItem::Ranges(items, false) => ranges.extend_from_slice(items),
        ClassItem::Ranges(items, true) => ranges.extend(complement(items)),
    }
}

// A class, from the character after its `[`; returns the index after its
// `]`
fn class(
    chars: &[char],
    mut i: usize,
    out: &mut String,
    ignore_case: bool,
) -> Result<usize, String> {
    let is_negated = chars.get(i) == Some(&'^');
    if is_negated {
        i += 1;
    }
    let mut ranges = Vec::new();
    loop {
        match chars.get(i) {
            None => return Err("unterminated character class".to_owned()),
            Some(']') => {
                push_class(out, ranges, is_negated, ignore_case);
                return Ok(i + 1);
            }
            Some(_) => {}
        }
        let (first, next) = class_item(chars, i)?;
        i = next;
        // A range, unless `-` is last or a side is a class escape: then `-`
        // is a literal
        if chars.get(i) == Some(&'-') && chars.get(i + 1).is_some_and(|char| *char != ']') {
            let (last, next) = class_item(chars, i + 1)?;
            i = next;
            match (first, last) {
                (ClassItem::Char(start), ClassItem::Char(end)) => {
                    if start > end {
                        return Err("range out of order in character class".to_owned());
                    }
                    ranges.push((start, end));
                }
                (first, last) => {
                    add_item(&mut ranges, first);
                    add_item(&mut ranges, ClassItem::Char(u32::from('-')));
                    add_item(&mut ranges, last);
                }
            }
        } else {
            add_item(&mut ranges, first);
        }
    }
}

fn class_item(chars: &[char], i: usize) -> Result<(ClassItem, usize), String> {
    match chars[i] {
        '\\' => match chars.get(i + 1) {
            None => Err("\\ at the end of the pattern".to_owned()),
            Some(&escape @ ('d' | 'D' | 'w' | 'W' | 's' | 'S')) => {
                let (ranges, is_negated) = class_escape(escape);
                Ok((ClassItem::Ranges(ranges, is_negated), i + 2))
            }
            Some(_) => {
                let (code, length) = char_escape(&chars[i + 1..], true)?;
                Ok((ClassItem::Char(code), i + 1 + length))
            }
        },
        char => Ok((ClassItem::Char(u32::from(char)), i + 1)),
    }
}

// The code points outside sorted ranges
fn complement(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut complement = Vec::with_capacity(ranges.len() + 1);
    let mut next = 0;
    for &(start, end) in ranges {
        if start > next {
            complement.push((next, start - 1));
        }
        next = end + 1;
    }
    if next <= 0x10_ffff {
        complement.push((next, 0x10_ffff));
    }
    complement
}

// Sorted, merged ranges without surrogates, which a `str` cannot have
fn normalize(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
        .into_iter()
        .flat_map(|(start, end)| {
            let below = (start < 0xd800).then(|| (start, end.min(0xd7ff)));
            let above = (end > 0xdfff).then(|| (start.max(0xe000), end));
            below.into_iter().chain(above)
        })
        .collect()
}

fn push_class(out: &mut String, ranges: Vec<(u32, u32)>, is_negated: bool, ignore_case: bool) {
    let ranges = if ignore_case {
        normalize(fold_case(&normalize(ranges)))
    } else {
        normalize(ranges)
    };
    if ranges.is_empty() {
        // `[]` matches nothing, `[^]` any character
        out.push_str(if is_negated {
            r"[\x{0}-\x{10ffff}]"
        } else {
            r"[^\x{0}-\x{10ffff}]"
        });
        return;
    }
    out.push('[');
    if is_negated {
        out.push('^');
    }
    for (start, end) in ranges {
        push_code(out, start);
        if end > start {
            out.push('-');
            push_code(out, end);
        }
    }
    out.push(']');
}

fn push_code(out: &mut String, code: u32) {
    use std::fmt::Write;
    let _ = write!(out, "\\x{{{code:x}}}");
}

// A literal character; with `ignore_case`, the characters it matches
fn push_char(out: &mut String, code: u32, ignore_case: bool) -> Result<(), String> {
    if ignore_case {
        let ranges = fold_case(&[(code, code)]);
        if ranges.len() > 1 {
            push_class(out, ranges, false, false);
            return Ok(());
        }
    }
    let char = char::from_u32(code).ok_or("surrogates are not supported")?;
    if r"\.+*?()|[]{}^$#&-~".contains(char) {
        out.push('\\');
    }
    out.push(char);
    Ok(())
}

// The ranges with the characters that match them when case is ignored.
//
// JavaScript without the `u` flag compares the canonical forms of
// characters (`Canonicalize` in ECMAScript): the upper case of a UTF-16
// unit when it is a single unit, except that a character beyond ASCII never
// becomes an ASCII one. So `k` matches `K` but not the Kelvin sign, and `s`
// does not match the long s (`ſ`), whose upper case is `S`.
fn fold_case(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let groups = &*CASE_GROUPS;
    let mut folded = ranges.to_vec();
    for &(start, end) in ranges {
        let first = groups.members.partition_point(|(code, _)| *code < start);
        for &(code, canonical) in &groups.members[first..] {
            if code > end {
                break;
            }
            folded.extend(
                groups.by_canonical[&canonical]
                    .iter()
                    .map(|&code| (code, code)),
            );
        }
    }
    folded
}

struct CaseGroups {
    // The characters that match others, sorted, with their canonical form
    members: Vec<(u32, u32)>,
    // The characters of each canonical form
    by_canonical: HashMap<u32, Vec<u32>>,
}

// Built on the first `like` with the `i` flag, in a pass over the 64K units
static CASE_GROUPS: LazyLock<CaseGroups> = LazyLock::new(|| {
    let mut by_canonical: HashMap<u32, Vec<u32>> = HashMap::new();
    for code in 0..=0xffff {
        if let Some(char) = char::from_u32(code) {
            let canonical = canonicalize(char);
            if canonical != code {
                by_canonical.entry(canonical).or_default().push(code);
            }
        }
    }
    for (&canonical, codes) in &mut by_canonical {
        if char::from_u32(canonical).is_some_and(|char| canonicalize(char) == canonical) {
            codes.push(canonical);
        }
    }
    let mut members: Vec<(u32, u32)> = by_canonical
        .iter()
        .flat_map(|(&canonical, codes)| codes.iter().map(move |&code| (code, canonical)))
        .collect();
    members.sort_unstable();
    CaseGroups {
        members,
        by_canonical,
    }
});

// `Canonicalize` of ECMAScript, for a pattern without the `u` flag
fn canonicalize(char: char) -> u32 {
    let code = u32::from(char);
    let mut upper = char.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(upper), None)
            if u32::from(upper) <= 0xffff && (code < 0x80 || u32::from(upper) >= 0x80) =>
        {
            u32::from(upper)
        }
        _ => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pattern, flags, input, and whether JavaScript matches. Every
    // expectation was checked with Node.js v26.7.0:
    // `node -e 'console.log(new RegExp(pattern, flags).test(input))'`
    const CASES: &[(&str, &str, &str, bool)] = &[
        (r"^\d{5}$", "", "01001", true),
        (r"^\d+$", "", "١٢٣", false),
        (r"[\d.]+", "", "1.5", true),
        (r"^[[a]+$", "", "[a[", true),
        (r"^[a&&b]+$", "", "&", true),
        (r"^[a~~b]+$", "", "~", true),
        (r"^a\/b$", "", "a/b", true),
        // `.` stops at line terminators only
        (r"^.$", "", "\n", false),
        (r"^.$", "", "\r", false),
        (r"^.$", "", "\u{2028}", false),
        (r"^.$", "", "\u{2029}", false),
        (r"^.$", "", "\u{85}", true),
        (r"^.$", "", "x", true),
        // `$` is the end of the value, not of a line
        (r"^a$", "", "a\n", false),
        // Case folding keeps ASCII and other characters apart
        (r"^\w$", "i", "\u{17f}", false),
        (r"^\w$", "i", "\u{212a}", false),
        (r"^\w+$", "i", "aZ_9", true),
        (r"^\W$", "i", "\u{212a}", true),
        (r"^\W$", "i", "k", false),
        (r"^\d$", "i", "\u{17f}", false),
        (r"^[a-z]$", "i", "\u{17f}", false),
        (r"^[a-z]$", "i", "\u{212a}", false),
        (r"^[a-z]+$", "i", "ABC", true),
        (r"^[^a-z]$", "i", "\u{212a}", true),
        (r"^[^a-z]$", "i", "K", false),
        (r"^k$", "i", "\u{212a}", false),
        (r"^k$", "i", "K", true),
        (r"^s$", "i", "\u{17f}", false),
        ("^\u{212a}$", "i", "k", false),
        ("^\u{17f}$", "i", "S", false),
        ("^щ$", "i", "Щ", true),
        ("^[а-я]+$", "i", "КИЇ", false),
        ("^[а-я]+$", "i", "КИЙ", true),
        ("^σ$", "i", "ς", true),
        ("^Σ$", "i", "ς", true),
        ("^ß$", "i", "ẞ", false),
        ("^abc$", "", "ABC", false),
        // `\s` is the whitespace of JavaScript
        (r"^\s$", "", "\u{feff}", true),
        (r"^\s$", "", "\u{85}", false),
        (r"^\s$", "", "\u{3000}", true),
        (r"^\S$", "", "\u{85}", true),
        (r"^[\s]$", "", "\u{feff}", true),
        // Letters escaped without a meaning are literals
        (r"^\z$", "", "z", true),
        (r"^\A$", "", "A", true),
        (r"^\a$", "", "a", true),
        (r"^\<$", "", "<", true),
        (r"^\e$", "", "e", true),
        (r"^\h$", "", "h", true),
        (r"^\K$", "", "K", true),
        (r"^\Z$", "", "Z", true),
        (r"^\k$", "", "k", true),
        (r"^\p{L}$", "", "p{L}", true),
        (r"^\p{L}$", "", "a", false),
        (r"^\P{L}$", "", "P{L}", true),
        // NUL, control characters, hexadecimal and Unicode escapes
        (r"^\0$", "", "\0", true),
        (r"^\cJ$", "", "\n", true),
        (r"^\cj$", "", "\n", true),
        (r"^\c1$", "", "\\c1", true),
        (r"^[\c1]$", "", "\u{11}", true),
        (r"^\x41$", "", "A", true),
        (r"^\x4$", "", "x4", true),
        (r"^\u0041$", "", "A", true),
        (r"^\uD83D\uDE00$", "", "😀", true),
        (r"^\u{2}$", "", "uu", true),
        (r"^[\b]$", "", "\u{8}", true),
        (r"^a\b", "", "a b", true),
        (r"^\w\b", "", "aщ", true),
        // A brace that is not a quantifier is a literal
        (r"^a{$", "", "a{", true),
        (r"^a{2}$", "", "aa", true),
        (r"^a{2,}$", "", "aaa", true),
        (r"^a{1,2}$", "", "aa", true),
        (r"^x{,2}$", "", "x{,2}", true),
        (r"^a}$", "", "a}", true),
        (r"^a]$", "", "a]", true),
        // `-` at the edges of a class and next to class escapes
        (r"^[a-]$", "", "-", true),
        (r"^[-a]$", "", "-", true),
        (r"^[\d-z]$", "", "-", true),
        (r"^[\d-z]$", "", "5", true),
        (r"^[\d-z]$", "", "y", false),
        (r"^[]$", "", "a", false),
        (r"^[^]$", "", "\n", true),
        (r"^[^\d]$", "", "a", true),
        (r"^[\W\d]$", "", "5", true),
        (r"^[\W\d]$", "", "a", false),
        (r"^[.]$", "", "x", false),
        // Groups
        (r"^(?:ab)+$", "", "abab", true),
        (r"^(?<year>\d{4})$", "", "2024", true),
        (r"^(a|b)$", "", "b", true),
    ];

    #[test]
    fn matches_like_javascript() {
        for &(pattern, flags, input, expected) in CASES {
            let like = Like::new(pattern, flags.contains('i'))
                .unwrap_or_else(|error| panic!("{pattern:?}: {error}"));
            assert_eq!(
                like.re.is_match(input),
                expected,
                "/{pattern}/{flags} on {input:?}, translated to {}",
                like.re.as_str()
            );
        }
    }

    // Each of these is a syntax error in JavaScript, or means something the
    // `regex` crate cannot do
    #[test]
    fn rejects_what_has_no_equivalent() {
        for pattern in [
            "(?=a)",
            "(?!a)",
            "(?<=a)",
            "(?<!a)",
            r"(a)\1",
            r"\01",
            "(?i:a)",
            "(?i)a",
            r"(?<a>x)\k<a>",
            r"\uD83D",
            r"[\uD83D]",
            "[z-a]",
            "[a",
            "a\\",
        ] {
            assert!(Like::new(pattern, false).is_err(), "{pattern:?}");
        }
    }
}
