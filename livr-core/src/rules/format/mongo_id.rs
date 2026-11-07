use crate::rules::prelude::*;

/// `mongo_id` (LIVR 2.5): 24 hex digits, a BSON ObjectId.
#[derive(Clone, Copy, Debug, Default)]
pub struct MongoId;

impl MongoId {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "mongo_id")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        check_format(value, "WRONG_MONGO_ID", |bytes| {
            bytes.len() == 24 && bytes.iter().all(|&byte| is_hex_digit(byte))
        })
    }
}
