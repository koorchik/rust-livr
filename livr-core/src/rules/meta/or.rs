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

    /// Whether a rule set reads other fields of the object.
    #[inline]
    #[must_use]
    pub fn reads_ctx(&self) -> bool {
        self.rule_sets.iter().any(Chain::reads_ctx)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        // The rule sets are transparent: their rules see the object of the
        // field (LIVR 2.5)
        let mut outcome = Outcome::Valid;
        for rules in &self.rule_sets {
            outcome = rules.check(value, ctx);
            if !outcome.is_error() {
                return outcome;
            }
        }
        outcome
    }
}
