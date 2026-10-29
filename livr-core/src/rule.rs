//! The rule contract: what a rule returns ([`Outcome`]), what it sees
//! ([`Ctx`]), and how custom rules are built ([`Rule`], [`RuleBuilder`]).

use serde_json::{Map, Value};

use crate::compile::CompileCtx;
use crate::error::{ErrorTree, SchemaError};

/// The result of a rule.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub enum Outcome {
    /// Valid, the value is unchanged. Allocates nothing.
    Valid,
    /// Valid, the value becomes this one.
    Value(Value),
    /// Invalid: an error code, or the errors of a metarule.
    Error(ErrorTree),
}

impl Outcome {
    /// An error with a static code, like `Outcome::code("REQUIRED")`.
    #[inline]
    pub const fn code(code: &'static str) -> Self {
        Self::Error(ErrorTree::code(code))
    }

    /// Whether the outcome is an error.
    #[inline]
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// What a rule sees besides its value: the other fields of the object.
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'a> {
    siblings: Siblings<'a>,
}

#[derive(Clone, Copy, Debug)]
enum Siblings<'a> {
    Object(&'a Map<String, Value>),
    // A value validated on its own (list items, `or` and aliases) is the
    // only field of its data: no map is built for it
    Single(&'a str, &'a Value),
    // Rules that read no other fields
    None,
}

impl<'a> Ctx<'a> {
    /// The context of a field of the object.
    #[inline]
    #[must_use]
    pub fn object(fields: &'a Map<String, Value>) -> Self {
        Self {
            siblings: Siblings::Object(fields),
        }
    }

    /// The context of a value validated on its own, as the field `name`.
    #[inline]
    #[must_use]
    pub fn single(name: &'a str, value: &'a Value) -> Self {
        Self {
            siblings: Siblings::Single(name, value),
        }
    }

    /// The context of rules that read no other fields: values validated with
    /// [`Chain::run`](crate::Chain::run) on owned values, when it does not
    /// [read the context](crate::Chain::reads_ctx).
    #[inline]
    #[must_use]
    pub fn none() -> Self {
        Self {
            siblings: Siblings::None,
        }
    }

    /// The raw value of a field of the same object, before any rule.
    #[inline]
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&'a Value> {
        match self.siblings {
            Siblings::Object(fields) => fields.get(name),
            Siblings::Single(field, value) => (field == name).then_some(value),
            Siblings::None => None,
        }
    }
}

/// A compiled rule: checks a value, never changes it in place.
///
/// Rules skip empty values ([`is_no_value`](crate::util::is_no_value)) unless
/// they are about them, like `required`.
pub trait Rule: Send + Sync {
    /// Checks the value.
    fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome;
}

impl<F> Rule for F
where
    F: Fn(&Value, &Ctx<'_>) -> Outcome + Send + Sync,
{
    #[inline]
    fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        self(value, ctx)
    }
}

impl Rule for Box<dyn Rule> {
    #[inline]
    fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        (**self).check(value, ctx)
    }
}

/// A rule from a closure, with its signature inferred.
///
/// ```
/// use livr_core::{Outcome, rule_fn};
/// let rule = rule_fn(|value, _ctx| {
///     if value == "secret" { Outcome::code("NOT_ALLOWED") } else { Outcome::Valid }
/// });
/// # let _ = rule;
/// ```
pub fn rule_fn<F>(check: F) -> F
where
    F: Fn(&Value, &Ctx<'_>) -> Outcome + Send + Sync,
{
    check
}

/// A rule builder from a closure, with its signature inferred.
///
/// ```
/// use livr_core::{Outcome, Registry, rule_builder, rule_fn, util::string_arg};
/// let mut registry = Registry::default();
/// registry.register_rule("starts_with", rule_builder(|args, _ctx| {
///     let prefix = string_arg(args, 0, "starts_with")?;
///     Ok(rule_fn(move |value, _ctx| match value.as_str() {
///         Some(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
///         _ => Outcome::Valid,
///     }))
/// }));
/// ```
pub fn rule_builder<F, R>(build: F) -> F
where
    F: Fn(&[Value], &mut CompileCtx<'_>) -> Result<R, SchemaError> + Send + Sync,
    R: Rule + 'static,
{
    build
}

/// Builds a rule from its arguments in the schema, once per validator.
/// Expensive work (parsing arguments, compiling nested rules with
/// [`CompileCtx`]) goes here, not in the rule.
///
/// Closures (see [`rule_builder`]) and functions are rule builders. With
/// edition 2024, the `impl Rule` of a function must not capture the lifetimes
/// of the arguments:
///
/// ```
/// use livr_core::{CompileCtx, Outcome, Rule, SchemaError, Value, rule_fn};
/// fn uuid(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<impl Rule + use<>, SchemaError> {
///     Ok(rule_fn(|value, _ctx| Outcome::Valid))
/// }
/// livr_core::Registry::default().register_rule("uuid", uuid);
/// ```
pub trait RuleBuilder: Send + Sync {
    /// Builds the rule.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for invalid arguments.
    fn build(&self, args: &[Value], ctx: &mut CompileCtx<'_>)
    -> Result<Box<dyn Rule>, SchemaError>;
}

impl<F, R> RuleBuilder for F
where
    F: Fn(&[Value], &mut CompileCtx<'_>) -> Result<R, SchemaError> + Send + Sync,
    R: Rule + 'static,
{
    fn build(
        &self,
        args: &[Value],
        ctx: &mut CompileCtx<'_>,
    ) -> Result<Box<dyn Rule>, SchemaError> {
        Ok(Box::new(self(args, ctx)?))
    }
}
