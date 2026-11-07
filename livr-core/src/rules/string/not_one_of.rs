use std::collections::HashSet;

use crate::rules::prelude::*;

// Up to this length, a scan over the strings is as fast as a set and cheaper
// to build, which matters when a validator is built for every validation
const MAX_LIST_FOR_SCAN: usize = 16;

/// `not_one_of` (LIVR 2.5): the value must not be one of the arguments,
/// compared as strings: `{"not_one_of": ["admin", "root"]}`. The output is
/// the value unchanged.
#[derive(Clone, Debug)]
pub struct NotOneOf {
    forbidden: Box<[Box<str>]>,
    // The forbidden strings, for long lists
    set: Option<HashSet<Box<str>>>,
}

impl NotOneOf {
    /// Builds the rule from its arguments, one or more primitive values.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without values or for a value that is not primitive.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        if args.is_empty() {
            return Err(SchemaError::new(
                "rule [not_one_of] takes one or more values",
            ));
        }
        let forbidden: Box<[Box<str>]> = args
            .iter()
            .map(|value| {
                if is_primitive(value) {
                    Ok(to_js_string(value).into())
                } else {
                    Err(SchemaError::new(format!(
                        "rule [not_one_of] takes strings, numbers or booleans, got {value}"
                    )))
                }
            })
            .collect::<Result<_, _>>()?;
        let set =
            (forbidden.len() > MAX_LIST_FOR_SCAN).then(|| forbidden.iter().cloned().collect());
        Ok(Self { forbidden, set })
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
        let is_forbidden = match &self.set {
            Some(set) => set.contains(&*str),
            None => self.forbidden.iter().any(|forbidden| **forbidden == *str),
        };
        if is_forbidden {
            Outcome::code("NOT_ALLOWED_VALUE")
        } else {
            Outcome::Valid
        }
    }
}
