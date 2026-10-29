use std::collections::HashMap;

use crate::rules::prelude::*;

/// `list_of_different_objects`: the value must be a list of objects, each
/// valid for the schema selected by the value of a field:
/// `{"list_of_different_objects": ["type", {"a": {schema}, "b": {schema}}]}`.
#[derive(Debug)]
pub struct ListOfDifferentObjects {
    selector: String,
    objects: HashMap<String, ObjectValidator>,
}

impl ListOfDifferentObjects {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for invalid arguments or schemas.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let selector = string_arg(args, 0, "list_of_different_objects")?;
        let Value::Object(schemas) = arg(args, 1, "list_of_different_objects")? else {
            return Err(SchemaError::new(
                "rule [list_of_different_objects] expects schemas by selector value",
            ));
        };
        let objects = schemas
            .iter()
            .map(|(value, schema)| Ok((value.clone(), ctx.compile_object(schema)?)))
            .collect::<Result<_, SchemaError>>()?;
        Ok(Self { selector, objects })
    }

    // The schema for the value of the selector field of an item: a primitive
    // value listed in the schemas
    fn object(&self, item: &Value) -> Option<&ObjectValidator> {
        match item.as_object()?.get(&self.selector) {
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
        match &*value {
            Value::Array(_) => run_items(value, |item| match self.object(&item) {
                Some(object) => object.run(item),
                None => Err(ErrorTree::code("FORMAT_ERROR")),
            }),
            _ if is_no_value(&value) => Ok(value),
            _ => Err(ErrorTree::code("FORMAT_ERROR")),
        }
    }
}
