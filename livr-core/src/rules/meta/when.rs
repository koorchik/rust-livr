use std::collections::HashMap;

use crate::rules::prelude::*;

/// `when` (LIVR 2.5): rules that depend on another field. The condition form
/// `{"when": [REFERENCE, CONDITION, RULES_IF_TRUE, RULES_IF_FALSE]}` (the last
/// one optional), or the switch form `{"when": [REFERENCE, CASES]}`. The
/// branch gets the value as it is, empty values included.
#[derive(Debug)]
pub struct When {
    reference: Reference,
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    Condition {
        condition: Chain,
        if_true: Chain,
        // No rules without the 4th argument
        if_false: Chain,
    },
    Switch(HashMap<String, Chain>),
}

impl When {
    /// Builds the rule from its arguments: 2 for the switch form, 3 or 4 for
    /// the condition form; arguments after the 4th are ignored.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for fewer than 2 arguments, a wrong reference,
    /// cases that are not an object or are empty, an empty list of rules, or
    /// invalid rules.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let kind = match args {
            [_, cases] => {
                let cases = match cases {
                    Value::Object(cases) if !cases.is_empty() => cases,
                    Value::Object(_) => {
                        return Err(SchemaError::new(
                            "the cases of rule [when] must not be empty",
                        ));
                    }
                    _ => {
                        return Err(SchemaError::new(format!(
                            "the cases of rule [when] must be an object of values to rules, got {cases}"
                        )));
                    }
                };
                let cases = cases
                    .iter()
                    .map(|(case, rules)| Ok((case.clone(), compile_rules(rules, ctx)?)))
                    .collect::<Result<_, SchemaError>>()?;
                Kind::Switch(cases)
            }
            [_, condition, if_true, rest @ ..] => Kind::Condition {
                condition: compile_rules(condition, ctx)?,
                if_true: compile_rules(if_true, ctx)?,
                if_false: match rest.first() {
                    Some(if_false) => compile_rules(if_false, ctx)?,
                    None => ctx.compile_field(&Value::Array(Vec::new()))?,
                },
            },
            _ => {
                return Err(SchemaError::new(format!(
                    "rule [when] takes a reference and cases, or a reference, a condition, rules if true and optional rules if false, got {} arguments",
                    args.len()
                )));
            }
        };
        Ok(Self {
            reference: Reference::parse(&args[0], "when")?,
            kind,
        })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        outcome(self.run(Cow::Borrowed(value), ctx))
    }

    /// Runs the rule on a borrowed or an owned value.
    #[inline]
    pub fn run<'v>(&self, value: Cow<'v, Value>, ctx: &Ctx<'_>) -> Checked<'v> {
        let other = self.reference.resolve(ctx);
        match &self.kind {
            Kind::Condition {
                condition,
                if_true,
                if_false,
            } => {
                // The condition rules see the object of the field, their
                // output is discarded
                let is_true = other.is_some_and(|other| {
                    !is_no_value(other) && !condition.check(other, ctx).is_error()
                });
                if is_true { if_true } else { if_false }.run(value, ctx)
            }
            Kind::Switch(cases) => {
                let case = other
                    .filter(|other| is_primitive(other) && !is_no_value(other))
                    .and_then(|other| cases.get(&*to_js_string(other)));
                match case {
                    Some(rules) => rules.run(value, ctx),
                    // No case: an empty value passes, any other one would
                    // pass unchecked
                    None if is_no_value(&value) => Ok(value),
                    None => Err(ErrorTree::code("NOT_ALLOWED_VALUE")),
                }
            }
        }
    }
}

// The rules of a condition, a branch or a case: an empty list is almost
// always a mistake, "any" is the branch without rules
fn compile_rules(rules: &Value, ctx: &mut CompileCtx<'_>) -> Result<Chain, SchemaError> {
    if matches!(rules, Value::Array(rules) if rules.is_empty()) {
        return Err(SchemaError::new(
            "rule [when] got an empty list of rules, use \"any\" for no rules",
        ));
    }
    ctx.compile_field(rules)
}
