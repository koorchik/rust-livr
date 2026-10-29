use crate::rules::prelude::*;

/// `or`: the value must be valid for one of the rule sets, tried in order:
/// `{"or": [["required", "email"], ["required", "positive_integer"]]}`. The
/// output is the one of the first valid set, the error the one of the last.
#[derive(Debug)]
pub struct Or {
    rule_sets: Box<[Chain]>,
}

impl Or {
    /// Builds the rule from its arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for invalid rules.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let rule_sets = args
            .iter()
            .map(|rules| ctx.compile_field(rules))
            .collect::<Result<_, _>>()?;
        Ok(Self { rule_sets })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        // Rules of a set see the value as the only field of the data
        let ctx = Ctx::single("field", value);
        let mut outcome = Outcome::Valid;
        for rules in &self.rule_sets {
            outcome = rules.check(value, &ctx);
            if !outcome.is_error() {
                return outcome;
            }
        }
        outcome
    }
}
