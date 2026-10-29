use std::collections::HashMap;

use crate::rules::prelude::*;

/// `variable_object`: the value must be an object valid for the schema
/// selected by the value of a field:
/// `{"variable_object": ["type", {"a": {schema}, "b": {schema}}]}`.
#[derive(Debug)]
pub struct VariableObject {
    selector: String,
    objects: HashMap<String, ObjectValidator>,
}

impl VariableObject {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for invalid arguments or schemas.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let selector = string_arg(args, 0, "variable_object")?;
        let Value::Object(schemas) = arg(args, 1, "variable_object")? else {
            return Err(SchemaError::new(
                "rule [variable_object] expects schemas by selector value",
            ));
        };
        let objects = schemas
            .iter()
            .map(|(value, schema)| Ok((value.clone(), ctx.compile_object(schema)?)))
            .collect::<Result<_, SchemaError>>()?;
        Ok(Self { selector, objects })
    }

    // The schema for the value of the selector field: a primitive value
    // listed in the schemas
    fn object(&self, value: &Value) -> Option<&ObjectValidator> {
        match value.get(&self.selector) {
            Some(selector) if is_primitive(selector) && !is_no_value(selector) => {
                self.objects.get(&*to_js_string(selector))
            }
            _ => None,
        }
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
            return Ok(value);
        }
        match value.is_object().then(|| self.object(&value)).flatten() {
            Some(object) => object.run(value),
            None => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }
}
