use crate::rules::prelude::*;

/// `list_of_objects`: the value must be a list of objects valid for the
/// schema.
#[derive(Debug)]
pub struct ListOfObjects {
    object: ObjectValidator,
}

impl ListOfObjects {
    /// Builds the rule from its arguments: `{"list_of_objects": {schema}}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid schema.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self {
            object: ctx.compile_object(arg(args, 0, "list_of_objects")?)?,
        })
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
            Value::Array(_) => run_items(value, |item| self.object.run(item)),
            _ if is_no_value(&value) => Ok(value),
            _ => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }
}
