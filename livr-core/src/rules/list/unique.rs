use std::collections::HashSet;

use crate::rules::prelude::*;

/// `unique` (LIVR 2.5): the items of the list must be unique, compared as
/// strings: `"unique"` for primitive items, `{"unique": ["country", "zip"]}`
/// for objects with unique values of the keys. Empty items, and items with
/// an empty key value, are ignored.
#[derive(Clone, Debug)]
pub struct Unique {
    keys: Box<[String]>,
}

impl Unique {
    /// Builds the rule from its arguments: none, or key names.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for a key that is not a string.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let keys = args
            .iter()
            .map(|key| match text_arg(key) {
                Some(key) => Ok(key.into_owned()),
                None => Err(SchemaError::new(format!(
                    "rule [unique] takes key names as strings, got {key}"
                ))),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { keys })
    }

    /// Checks the value. A wrong item gives `FORMAT_ERROR` wherever it is,
    /// even after duplicates.
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let items = match value {
            Value::Array(items) => items,
            value if is_no_value(value) => return Outcome::Valid,
            _ => return Outcome::code("FORMAT_ERROR"),
        };
        let mut has_duplicates = false;
        if self.keys.is_empty() {
            let mut seen = HashSet::with_capacity(items.len());
            for item in items {
                if is_no_value(item) {
                    continue;
                }
                if !is_primitive(item) {
                    return Outcome::code("FORMAT_ERROR");
                }
                has_duplicates |= !seen.insert(to_js_string(item));
            }
        } else {
            let mut seen = HashSet::with_capacity(items.len());
            for item in items {
                let fields = match item {
                    Value::Object(fields) => fields,
                    item if is_no_value(item) => continue,
                    _ => return Outcome::code("FORMAT_ERROR"),
                };
                let mut key_values = Vec::with_capacity(self.keys.len());
                let mut has_empty_value = false;
                for key in &self.keys {
                    match fields.get(key) {
                        None => has_empty_value = true,
                        Some(value) if is_no_value(value) => has_empty_value = true,
                        Some(value) if is_primitive(value) => key_values.push(to_js_string(value)),
                        Some(_) => return Outcome::code("FORMAT_ERROR"),
                    }
                }
                if !has_empty_value {
                    has_duplicates |= !seen.insert(key_values);
                }
            }
        }
        if has_duplicates {
            Outcome::code("NOT_UNIQUE_ITEMS")
        } else {
            Outcome::Valid
        }
    }
}
