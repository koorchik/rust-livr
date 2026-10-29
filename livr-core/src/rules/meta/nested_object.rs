use crate::rules::prelude::*;

/// `nested_object`: the value must be an object valid for the schema.
#[derive(Debug)]
pub struct NestedObject {
    object: ObjectValidator,
}

impl NestedObject {
    /// Builds the rule from its arguments: `{"nested_object": {schema}}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid schema.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self {
            object: ctx.compile_object(arg(args, 0, "nested_object")?)?,
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
        if is_no_value(&value) {
            Ok(value)
        } else {
            self.object.run(value)
        }
    }
}
