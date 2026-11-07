use std::collections::BTreeMap;

use crate::rules::prelude::*;

/// `values` (LIVR 2.5): the value must be an object with any keys, each of
/// its values valid for the rules: `{"values": ["required", "positive_integer"]}`.
/// The arguments are the ones of `list_of`.
#[derive(Debug)]
pub struct Values {
    rules: Chain,
}

impl Values {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without rules or for invalid rules.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let rules = match args {
            [] => return Err(SchemaError::new("rule [values] needs rules")),
            [Value::Array(rules)] if rules.is_empty() => {
                return Err(SchemaError::new("rule [values] needs rules"));
            }
            [rules @ Value::Array(_)] => ctx.compile_field(rules)?,
            rules => ctx.compile_field(&Value::Array(rules.to_vec()))?,
        };
        Ok(Self { rules })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        outcome(self.run(Cow::Borrowed(value)))
    }

    /// Runs the rule on a borrowed or an owned value. Values have no object
    /// of their own: their rules see no other fields.
    pub fn run<'v>(&self, value: Cow<'v, Value>) -> Checked<'v> {
        let mut errors: Option<BTreeMap<String, ErrorTree>> = None;
        match value {
            Cow::Borrowed(object @ Value::Object(fields)) => {
                // A new object only when a value changes
                let mut output: Option<Map<String, Value>> = None;
                for (i, (key, field)) in fields.iter().enumerate() {
                    match self.rules.run(Cow::Borrowed(field), &Ctx::none()) {
                        Ok(Cow::Borrowed(_)) => {
                            if let Some(output) = &mut output {
                                output.insert(key.clone(), field.clone());
                            }
                        }
                        Ok(Cow::Owned(new_value)) => {
                            if errors.is_none() {
                                output
                                    .get_or_insert_with(|| {
                                        fields
                                            .iter()
                                            .take(i)
                                            .map(|(key, field)| (key.clone(), field.clone()))
                                            .collect()
                                    })
                                    .insert(key.clone(), new_value);
                            }
                        }
                        Err(error) => {
                            errors.get_or_insert_default().insert(key.clone(), error);
                            output = None;
                        }
                    }
                }
                match (errors, output) {
                    (Some(errors), _) => Err(ErrorTree::Object(errors)),
                    (None, Some(output)) => Ok(Cow::Owned(Value::Object(output))),
                    (None, None) => Ok(Cow::Borrowed(object)),
                }
            }
            // The values are validated in place
            Cow::Owned(Value::Object(mut fields)) => {
                for (key, field) in &mut fields {
                    match self
                        .rules
                        .run(Cow::Owned(std::mem::take(field)), &Ctx::none())
                    {
                        Ok(output) => *field = output.into_owned(),
                        Err(error) => {
                            errors.get_or_insert_default().insert(key.clone(), error);
                        }
                    }
                }
                match errors {
                    Some(errors) => Err(ErrorTree::Object(errors)),
                    None => Ok(Cow::Owned(Value::Object(fields))),
                }
            }
            value if is_no_value(&value) => Ok(value),
            _ => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }
}
