use std::collections::BTreeMap;

use crate::rules::prelude::*;

/// `keys` (LIVR 2.5): the value must be an object whose keys are valid for
/// the rules, as strings: `{"keys": {"one_of": ["uk", "en"]}}`. A wrong key
/// gives `WRONG_KEY` under that key. The rules only check: the keys do not
/// change. The arguments are the ones of `list_of`.
#[derive(Debug)]
pub struct Keys {
    rules: Chain,
}

impl Keys {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without rules or for invalid rules.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let rules = match args {
            [] => return Err(SchemaError::new("rule [keys] needs rules")),
            [Value::Array(rules)] if rules.is_empty() => {
                return Err(SchemaError::new("rule [keys] needs rules"));
            }
            [rules @ Value::Array(_)] => ctx.compile_field(rules)?,
            rules => ctx.compile_field(&Value::Array(rules.to_vec()))?,
        };
        Ok(Self { rules })
    }

    /// Checks the value. Keys have no object of their own: their rules see
    /// no other fields.
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let fields = match value {
            Value::Object(fields) => fields,
            value if is_no_value(value) => return Outcome::Valid,
            _ => return Outcome::code("FORMAT_ERROR"),
        };
        let mut errors: Option<BTreeMap<String, ErrorTree>> = None;
        // One string for all keys
        let mut key_value = Value::String(String::new());
        for key in fields.keys() {
            if let Value::String(str) = &mut key_value {
                str.clear();
                str.push_str(key);
            }
            if self.rules.check(&key_value, &Ctx::none()).is_error() {
                errors
                    .get_or_insert_default()
                    .insert(key.clone(), ErrorTree::code("WRONG_KEY"));
            }
        }
        match errors {
            Some(errors) => Outcome::Error(ErrorTree::Object(errors)),
            None => Outcome::Valid,
        }
    }
}
