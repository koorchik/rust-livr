//! The validator: a compiled schema, and its builder.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::compile::{CompileCtx, ObjectValidator};
use crate::error::{Errors, SchemaError, ValidateIntoError};
use crate::registry::Registry;
use crate::trim::{trim_deep, trim_deep_owned};

/// A compiled schema. Compiled once, immutable, cheap to clone and safe to
/// share between threads.
///
/// ```
/// use livr_core::Validator;
/// use serde_json::json;
/// let validator = Validator::new(&json!({
///     "name": "required",
///     "email": ["required", "email"],
///     "age": "positive_integer",
/// }))?;
/// let output = validator.validate(&json!({"name": "Jane", "email": "jane@example.com", "age": "42", "x": 1}));
/// assert_eq!(output, Ok(json!({"name": "Jane", "email": "jane@example.com", "age": 42})));
/// # Ok::<(), livr_core::SchemaError>(())
/// ```
#[derive(Clone)]
pub struct Validator {
    inner: Arc<Inner>,
}

struct Inner {
    object: ObjectValidator,
    auto_trim: bool,
}

impl Validator {
    /// A validator with the built-in rules.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid schema.
    pub fn new(schema: &Value) -> Result<Self, SchemaError> {
        Self::builder(schema).build()
    }

    /// A builder, for custom rules and options.
    pub fn builder(schema: &Value) -> ValidatorBuilder<'_> {
        ValidatorBuilder {
            schema,
            registry: None,
            auto_trim: false,
        }
    }

    /// Validates the data: the output with the fields of the schema only, or
    /// the errors.
    ///
    /// # Errors
    ///
    /// [`Errors::Format`] when the data is not an object, the errors of the
    /// fields otherwise.
    pub fn validate(&self, data: &Value) -> Result<Value, Errors> {
        let trimmed;
        let data = if self.inner.auto_trim {
            trimmed = trim_deep(data);
            trimmed.as_ref().unwrap_or(data)
        } else {
            data
        };
        let Value::Object(data) = data else {
            return Err(Errors::Format);
        };
        self.inner
            .object
            .validate_map(data)
            .map(Value::Object)
            .map_err(Errors::Fields)
    }

    /// Validates owned data, like data just parsed: the values of the fields
    /// are moved to the output instead of copied, so it is faster than
    /// [`validate`](Self::validate).
    ///
    /// # Errors
    ///
    /// [`Errors::Format`] when the data is not an object, the errors of the
    /// fields otherwise.
    pub fn validate_owned(&self, mut data: Value) -> Result<Value, Errors> {
        if self.inner.auto_trim {
            trim_deep_owned(&mut data);
        }
        let Value::Object(data) = data else {
            return Err(Errors::Format);
        };
        self.inner
            .object
            .validate_map_owned(data)
            .map(Value::Object)
            .map_err(Errors::Fields)
    }

    /// Validates the data and deserializes the output.
    ///
    /// # Errors
    ///
    /// [`ValidateIntoError::Invalid`] for invalid data,
    /// [`ValidateIntoError::Convert`] when the output does not fit `T`.
    pub fn validate_into<T: DeserializeOwned>(&self, data: &Value) -> Result<T, ValidateIntoError> {
        let output = self.validate(data).map_err(ValidateIntoError::Invalid)?;
        serde_json::from_value(output).map_err(ValidateIntoError::Convert)
    }

    /// Whether strings are trimmed before validation.
    #[must_use]
    pub fn auto_trim(&self) -> bool {
        self.inner.auto_trim
    }
}

// A validator holds custom rules as `Box<dyn Rule>`, which are not unwind
// safe by their type. Rules are immutable after they are built (`check`
// takes `&self`, and `Rule: Sync` leaves only locks and atomics for state),
// so a panic in a rule cannot leave a validator half changed.
impl std::panic::UnwindSafe for Validator {}
impl std::panic::RefUnwindSafe for Validator {}

impl std::fmt::Debug for Validator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Validator")
            .field("auto_trim", &self.inner.auto_trim)
            .finish_non_exhaustive()
    }
}

/// Builds a [`Validator`]: `Validator::builder(&schema).registry(&registry).auto_trim(true).build()`.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct ValidatorBuilder<'a> {
    schema: &'a Value,
    registry: Option<&'a Registry>,
    auto_trim: bool,
}

impl<'a> ValidatorBuilder<'a> {
    /// The rules of the validator, [`Registry::builtin`] by default.
    pub fn registry(mut self, registry: &'a Registry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Whether to trim all strings of the data before validation. Off by
    /// default.
    pub fn auto_trim(mut self, auto_trim: bool) -> Self {
        self.auto_trim = auto_trim;
        self
    }

    /// Compiles the schema.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid schema.
    pub fn build(self) -> Result<Validator, SchemaError> {
        let registry = self.registry.unwrap_or_else(|| Registry::builtin());
        let object = CompileCtx::new(registry).compile_object(self.schema)?;
        Ok(Validator {
            inner: Arc::new(Inner {
                object,
                auto_trim: self.auto_trim,
            }),
        })
    }
}
