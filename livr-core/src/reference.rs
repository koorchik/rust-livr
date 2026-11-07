//! References to other fields (LIVR 2.5, `required_if` and `when`): `"name"`
//! or a path `"a/b/0"` down from the containing object, with `~1` for `/` and
//! `~0` for `~` as in JSON Pointer.

use serde_json::Value;

use crate::error::SchemaError;
use crate::rule::Ctx;

/// A parsed reference.
#[derive(Clone, Debug)]
pub(crate) struct Reference {
    // The first segment is a field of the containing object
    first: Box<str>,
    // The segments below it, with their index for lists
    rest: Box<[Segment]>,
}

#[derive(Clone, Debug)]
struct Segment {
    key: Box<str>,
    // `0` or `[1-9][0-9]*`: a list index
    index: Option<usize>,
}

impl Reference {
    /// Parses a reference of a rule.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for a value that is not a string, an empty string, a
    /// leading or trailing `/`, an empty segment or a wrong escape.
    pub(crate) fn parse(reference: &Value, rule: &str) -> Result<Self, SchemaError> {
        let Value::String(reference) = reference else {
            return Err(SchemaError::new(format!(
                "a reference of rule [{rule}] must be a string, got {reference}"
            )));
        };
        Self::parse_str(reference, rule)
    }

    /// Parses a reference given as a string (a key of `required_if`).
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] as in [`parse`](Self::parse).
    pub(crate) fn parse_str(reference: &str, rule: &str) -> Result<Self, SchemaError> {
        let error = |what: &str| {
            SchemaError::new(format!(
                "the reference {reference:?} of rule [{rule}] {what}"
            ))
        };
        if reference.is_empty() {
            return Err(error("is empty"));
        }
        let mut segments = reference.split('/').map(|segment| {
            if segment.is_empty() {
                return Err(error("has an empty segment (a leading \"/\" is reserved)"));
            }
            unescape(segment).ok_or_else(|| error("has a wrong escape"))
        });
        let first = segments.next().expect("one segment")?;
        let rest = segments
            .map(|key| {
                let key = key?;
                let is_index = key == "0"
                    || (key.as_bytes()[0] != b'0' && key.bytes().all(|byte| byte.is_ascii_digit()));
                let index = if is_index { key.parse().ok() } else { None };
                Ok(Segment {
                    key: key.into(),
                    index,
                })
            })
            .collect::<Result<_, SchemaError>>()?;
        Ok(Self {
            first: first.into(),
            rest,
        })
    }

    /// The input value at the path, `None` when a segment is missing or a
    /// value on the way is not an object (or a list for an index).
    #[inline]
    pub(crate) fn resolve<'a>(&self, ctx: &Ctx<'a>) -> Option<&'a Value> {
        let mut value = ctx.field(&self.first)?;
        for segment in &self.rest {
            value = match value {
                Value::Object(fields) => fields.get(&*segment.key)?,
                Value::Array(items) => items.get(segment.index?)?,
                _ => return None,
            };
        }
        Some(value)
    }
}

// `~1` is `/`, `~0` is `~`; any other `~` is wrong
fn unescape(segment: &str) -> Option<String> {
    if !segment.contains('~') {
        return Some(segment.to_owned());
    }
    let mut out = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    while let Some(char) = chars.next() {
        if char == '~' {
            match chars.next() {
                Some('0') => out.push('~'),
                Some('1') => out.push('/'),
                _ => return None,
            }
        } else {
            out.push(char);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolve(reference: &str, data: &Value) -> Option<Value> {
        let reference = Reference::parse_str(reference, "test").unwrap();
        reference
            .resolve(&Ctx::object(data.as_object().unwrap()))
            .cloned()
    }

    #[test]
    fn resolves_paths() {
        let data = json!({
            "a": {"b": [{"c": 1}, 2]},
            "x/y": "slash",
            "t~": "tilde",
            "list": [1, 2],
        });
        assert_eq!(resolve("a/b/0/c", &data), Some(json!(1)));
        assert_eq!(resolve("a/b/1", &data), Some(json!(2)));
        assert_eq!(resolve("a/b/01", &data), None);
        assert_eq!(resolve("a/b/2", &data), None);
        assert_eq!(resolve("a/b/c", &data), None);
        assert_eq!(resolve("x~1y", &data), Some(json!("slash")));
        assert_eq!(resolve("t~0", &data), Some(json!("tilde")));
        assert_eq!(resolve("list/1/x", &data), None);
        assert_eq!(resolve("missing", &data), None);
    }

    #[test]
    fn rejects_wrong_references() {
        for reference in ["", "/a", "a/", "a//b", "/", "a~2", "a~", "~"] {
            assert!(
                Reference::parse_str(reference, "test").is_err(),
                "{reference:?}"
            );
        }
        assert!(Reference::parse(&json!(1), "test").is_err());
    }
}
