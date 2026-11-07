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
            Value::Array(_) => run_items(value, |item| self.run_item(item)),
            _ if is_no_value(&value) => Ok(value),
            _ => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }

    // Items have no object of their own: their rules see no other fields
    // (LIVR 2.5). Not inlined: with the rules inlined into the loop of
    // `run_items`, the loop needs a much larger stack frame and the nested
    // benchmark was 10-15% slower
    #[inline(never)]
    fn run_item<'v>(&self, item: Cow<'v, Value>) -> Checked<'v> {
        self.item.run(item, &Ctx::none())
    }
}
