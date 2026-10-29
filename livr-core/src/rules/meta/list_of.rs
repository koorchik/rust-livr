use crate::rules::prelude::*;

/// `list_of`: the value must be a list of items valid for the rules:
/// `{"list_of": "positive_integer"}`, `{"list_of": ["required", "email"]}`,
/// or the old `{"list_of": [["required", "email"]]}`.
#[derive(Debug)]
pub struct ListOf {
    item: Chain,
}

impl ListOf {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for invalid rules.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let item = match args {
            [rules @ Value::Array(_)] => ctx.compile_field(rules)?,
            rules => ctx.compile_field(&Value::Array(rules.to_vec()))?,
        };
        Ok(Self { item })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        outcome(self.run(Cow::Borrowed(value)))
    }

    /// Runs the rule on a borrowed or an owned value.
    #[inline]
    pub fn run<'v>(&self, value: Cow<'v, Value>) -> Checked<'v> {
        match &*value {
            Value::Array(_) => {
                // Rules of an item see it as the only field of the data
                run_items(value, |item| self.item.run_single("field", item))
            }
            _ if is_no_value(&value) => Ok(value),
            _ => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }
}
