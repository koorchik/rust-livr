use std::collections::HashMap;

use crate::rules::prelude::*;

// Up to this length, a scan over the strings is as fast as a map and cheaper
// to build, which matters when a validator is built for every validation
const MAX_LIST_FOR_SCAN: usize = 16;

/// `one_of`: the value must be one of the arguments, compared as strings. The
/// output is the matching argument. Both `{"one_of": [a, b]}` and the old
/// `{"one_of": [[a, b]]}` are supported.
#[derive(Clone, Debug)]
pub struct OneOf {
    allowed: Box<[Value]>,
    // The allowed values as strings, when they are not all strings
    allowed_strs: Option<Box<[String]>>,
    // The index of the first allowed value by its string, for long lists
    index: Option<HashMap<String, usize>>,
}

impl OneOf {
    /// The rule for primitive values.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when a value is not primitive.
    pub fn new(allowed: Vec<Value>) -> Result<Self, SchemaError> {
        if let Some(value) = allowed.iter().find(|value| !is_primitive(value)) {
            return Err(SchemaError::new(format!(
                "rule [one_of] expects primitive values, got {value}"
            )));
        }
        let allowed_strs = (!allowed.iter().all(Value::is_string)).then(|| {
            allowed
                .iter()
                .map(|value| to_js_string(value).into_owned())
                .collect()
        });
        let mut rule = Self {
            allowed: allowed.into_boxed_slice(),
            allowed_strs,
            index: None,
        };
        if rule.allowed.len() > MAX_LIST_FOR_SCAN {
            let mut index = HashMap::with_capacity(rule.allowed.len());
            for i in 0..rule.allowed.len() {
                index.entry(rule.allowed_str(i).to_owned()).or_insert(i);
            }
            rule.index = Some(index);
        }
        Ok(rule)
    }

    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when a value is not primitive.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        match args {
            [Value::Array(allowed)] => Self::new(allowed.clone()),
            allowed => Self::new(allowed.to_vec()),
        }
    }

    #[inline]
    fn allowed_str(&self, i: usize) -> &str {
        match (&self.allowed_strs, &self.allowed[i]) {
            (Some(strs), _) => &strs[i],
            (None, Value::String(str)) => str,
            (None, _) => unreachable!("all allowed values are strings"),
        }
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
        let found = match &self.index {
            Some(index) => index.get(&*str).copied(),
            None => (0..self.allowed.len()).find(|&i| self.allowed_str(i) == str),
        };
        match found {
            None => Outcome::code("NOT_ALLOWED_VALUE"),
            Some(i) if self.allowed[i] == *value => Outcome::Valid,
            Some(i) => Outcome::Value(self.allowed[i].clone()),
        }
    }
}
