//! Auto-trim: all strings of the data trimmed before validation.
//!
//! The data is walked with a stack of its own, not by recursion: a `Value`
//! built in code has no depth limit (serde_json limits only parsed data), and
//! data must not overflow the stack.

use serde_json::Value;

use crate::util::{is_js_whitespace, js_trim};

/// The value with all strings trimmed, in nested objects and lists too, or
/// `None` when there is nothing to trim: then nothing is copied.
pub(crate) fn trim_deep(value: &Value) -> Option<Value> {
    if !has_untrimmed(value) {
        return None;
    }
    let mut copy = value.clone();
    trim_deep_owned(&mut copy);
    Some(copy)
}

// Whether a string of the value, at any depth, has whitespace to trim
fn has_untrimmed(value: &Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::String(str) => {
                if js_trim(str).len() != str.len() {
                    return true;
                }
            }
            Value::Array(items) => stack.extend(items),
            Value::Object(fields) => stack.extend(fields.values()),
            _ => {}
        }
    }
    false
}

/// Trims all strings of owned data in place: only changed strings are
/// touched, nothing is copied.
pub(crate) fn trim_deep_owned(value: &mut Value) {
    // Strings are trimmed when they are found, only lists and objects wait
    // on the stack: no allocation for data without nested ones
    let mut stack: Vec<&mut Value> = Vec::new();
    let mut next = Some(value);
    while let Some(value) = next.take().or_else(|| stack.pop()) {
        match value {
            Value::String(str) => trim_string(str),
            Value::Array(items) => trim_children(items.iter_mut(), &mut stack),
            Value::Object(fields) => trim_children(fields.values_mut(), &mut stack),
            _ => {}
        }
    }
}

// Trims the strings among the children, puts the lists and objects on the
// stack
fn trim_children<'a>(
    children: impl Iterator<Item = &'a mut Value>,
    stack: &mut Vec<&'a mut Value>,
) {
    for child in children {
        match child {
            Value::String(str) => trim_string(str),
            Value::Array(_) | Value::Object(_) => stack.push(child),
            _ => {}
        }
    }
}

fn trim_string(str: &mut String) {
    let start = str.len() - str.trim_start_matches(is_js_whitespace).len();
    let end = str.trim_end_matches(is_js_whitespace).len();
    if start > 0 || end < str.len() {
        // All whitespace: the end is before the start
        str.truncate(end.max(start));
        str.drain(..start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trims_nested_strings() {
        let data = json!({"a": " x ", "b": [1, " y", {"c": "z "}], "d": "ok"});
        assert_eq!(
            trim_deep(&data),
            Some(json!({"a": "x", "b": [1, "y", {"c": "z"}], "d": "ok"}))
        );
        assert_eq!(trim_deep(&json!({"a": ["x", 1]})), None);
        for data in [
            json!({"a": " x ", "b": [1, " y", {"c": "z "}], "d": "ok"}),
            json!({"a": ["x", 1], "b": "   ", "c": "\u{feff}\u{a0} a b\u{3000}\n", "d": "\u{85}a"}),
            json!(" top "),
            json!([[[" deep "]], {"x": [" y "]}]),
        ] {
            let mut owned = data.clone();
            trim_deep_owned(&mut owned);
            assert_eq!(owned, trim_deep(&data).unwrap_or(data));
        }
    }

    // Lists nested far deeper than serde_json would parse, as code can build
    // them: trimming must not overflow the stack
    #[test]
    fn trims_deep_data_without_recursion() {
        const DEPTH: usize = 200_000;
        let mut data = json!(" x ");
        for _ in 0..DEPTH {
            data = Value::Array(vec![data]);
        }
        assert!(has_untrimmed(&data));
        trim_deep_owned(&mut data);
        assert!(!has_untrimmed(&data));
        // Taken apart by hand: the drop of serde_json values is recursive
        let mut depth = 0;
        let mut value = data;
        while let Value::Array(mut items) = value {
            value = items.pop().expect("one item");
            depth += 1;
        }
        assert_eq!((depth, value), (DEPTH, json!("x")));
    }
}
