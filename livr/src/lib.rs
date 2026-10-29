//! A Rust implementation of the LIVR 2.0 specification (Language Independent
//! Validation Rules, <https://livr-spec.org>). It passes the full official
//! test suite: all 70 cases, also with camelCase rule names and as typed
//! schemas. LIVR rules are declarative and the same in all languages.
//!
//! - **All errors in one pass:** every field is checked, in nested objects
//!   and lists too, and gets the error code of its first failing rule.
//! - **Error codes shaped like the data:** `{"address": {"zip": "REQUIRED"}}`,
//!   the same codes in every LIVR implementation.
//! - **Strict:** fields without rules are not in the output.
//! - **Cleans as it validates:** `trim`, `to_lc`, `default`; `"42"` from a
//!   form becomes `42`.
//!
//! # Schemas known at run time: [`Validator`]
//!
//! ```
//! use livr::Validator;
//! use serde_json::json;
//!
//! let validator = Validator::new(&json!({
//!     "name": ["required", {"max_length": 20}],
//!     "email": ["required", "email"],
//!     "gender": {"one_of": ["male", "female"]},
//! }))?;
//!
//! let errors = validator.validate(&json!({"email": "wrong", "gender": "x"})).unwrap_err();
//! assert_eq!(errors.to_string(), r#"{"email":"WRONG_EMAIL","gender":"NOT_ALLOWED_VALUE","name":"REQUIRED"}"#);
//! # Ok::<(), livr::SchemaError>(())
//! ```
//!
//! - [`Validator::new`] compiles the schema once; an invalid schema is a
//!   [`SchemaError`] there, never during validation. A validator is
//!   immutable, `Send + Sync` and cheap to clone.
//! - [`Validator::validate`] borrows the data, [`Validator::validate_owned`]
//!   moves the values of owned data (a parsed request body) instead of
//!   copying them, [`Validator::validate_into`] deserializes the output.
//! - [`Validator::builder`] sets custom rules and aliases (a [`Registry`])
//!   and auto-trim.
//! - The errors are [`Errors`]: `FORMAT_ERROR` when the data is not an
//!   object, else an [`ErrorTree`] per invalid field. They serialize to the
//!   JSON of LIVR errors.
//! - A schema is limited to [`MAX_DEPTH`] levels of nested rules and
//!   [`MAX_RULES`] rules with aliases expanded, as schemas can come from a
//!   database.
//!
//! # Schemas known at compile time: [`schema!`]
//!
//! `schema!` checks the schema at compile time and infers the types of the
//! output from the rules:
//!
//! ```
//! livr::schema! {
//!     #[derive(Debug, PartialEq)]
//!     pub struct User {
//!         name: ["required", {"max_length": 20}],
//!         age: "positive_integer",
//!         role: {"one_of": ["admin", "user"]},
//!     }
//! }
//!
//! let user = User::validate(&serde_json::json!({"name": "Jane", "age": "42", "role": "admin"}))?;
//! assert_eq!(user, User { name: "Jane".into(), age: Some(42), role: Some(UserRole::Admin) });
//! # Ok::<(), livr::Errors>(())
//! ```
//!
//! The main type implements [`Schema`], for code generic over schemas, like
//! the extractors of [livr-axum](https://github.com/koorchik/rust-livr-axum).
//!
//! # Custom rules
//!
//! A rule builder gets the arguments of the rule in the schema and returns a
//! [`Rule`]; [`rule_builder`] and [`rule_fn`] infer their signatures. The
//! helpers of [`util`] read the arguments. A rule returns an [`Outcome`]:
//! valid, valid with a new value, or an error. [`Ctx::field`] gives the raw
//! value of another field. Metarules compile nested rules in the builder,
//! with [`CompileCtx::compile_field`] and [`CompileCtx::compile_object`].
//!
//! ```
//! use livr::{Alias, Outcome, Registry, Validator, rule_builder, rule_fn, util::string_arg};
//! use serde_json::json;
//!
//! let mut registry = Registry::default();
//! registry.register_rule("starts_with", rule_builder(|args, _ctx| {
//!     let prefix = string_arg(args, 0, "starts_with")?;
//!     Ok(rule_fn(move |value, _ctx| match value.as_str() {
//!         Some(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
//!         _ => Outcome::Valid,
//!     }))
//! }));
//! registry.register_alias(Alias::new("adult_age", json!(["positive_integer", {"min_number": 18}])).with_error("WRONG_AGE"))?;
//!
//! let validator = Validator::builder(&json!({"code": {"starts_with": "X-"}, "age": "adult_age"}))
//!     .registry(&registry)
//!     .build()?;
//! let errors = validator.validate(&json!({"code": "Y-1", "age": 12})).unwrap_err();
//! assert_eq!(errors.to_string(), r#"{"age":"WRONG_AGE","code":"WRONG_PREFIX"}"#);
//! # Ok::<(), livr::SchemaError>(())
//! ```
//!
//! Every rule is also registered under its camelCase name (`minLength`).
//!
//! # Semantics
//!
//! The rules follow the LIVR specification 2.0 and its test suite. Where it
//! leaves something open, this implementation is strict: numbers in strings
//! are decimal only, booleans are not numbers, integers fit `i64`, lengths
//! count characters, and `like` matches like a JavaScript regular expression
//! without the `u` flag. The
//! [README](https://github.com/koorchik/rust-livr#semantics) has the
//! details.
//!
//! # Features
//!
//! - `macros` (default): [`schema!`].
//! - `preserve_order`: objects of the output keep the order of their fields
//!   (`serde_json/preserve_order`).
//!
//! # Crates
//!
//! This crate re-exports `livr-core` (the engine) and `livr-macros`
//! (`schema!`); use it rather than them. Items hidden from this
//! documentation are used by the code of `schema!` and are not part of the
//! public API.
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use livr_core::*;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
#[doc = include_str!("schema.md")]
pub use livr_macros::schema;

#[doc(hidden)]
pub mod __private {
    //! What the code of `schema!` uses: the rules of the fields, compiled
    //! ahead of time, and the validation of the fields of typed objects,
    //! without intermediate maps. Values are `Cow`: borrowed from borrowed
    //! data, moved from owned data.

    use std::borrow::Cow;
    use std::collections::BTreeMap;

    pub use serde;
    pub use std::sync::LazyLock;

    use crate::util::is_no_value;
    use crate::{
        Chain, Checked, CompileCtx, Ctx, ErrorTree, FromLivr, FromLivrRef, Outcome, Registry, Rule,
        SchemaError, TypedObject, Value,
    };

    static NULL: Value = Value::Null;

    /// A schema or rules embedded by `schema!`, checked at compile time.
    #[must_use]
    pub fn json(json: &str) -> Value {
        serde_json::from_str(json).expect("JSON checked by schema!")
    }

    /// A rule built from its arguments, checked at compile time. The
    /// registry is got only if the rule compiles other rules.
    pub fn build<R>(
        registry: fn() -> &'static Registry,
        args: &str,
        build: impl FnOnce(&[Value], &mut CompileCtx<'_>) -> Result<R, SchemaError>,
    ) -> R {
        let Value::Array(args) = json(args) else {
            unreachable!("arguments are a list")
        };
        build(&args, &mut CompileCtx::lazy(registry)).expect("arguments checked by schema!")
    }

    /// Rules compiled by the engine, checked at compile time.
    #[must_use]
    pub fn chain(registry: &Registry, rules: &str) -> Chain {
        CompileCtx::new(registry)
            .compile_field(&json(rules))
            .expect("rules checked by schema!")
    }

    /// Runs a rule on a value, like a `Chain` does.
    #[inline(always)]
    pub fn step<'v, R: Rule + ?Sized>(
        rule: &R,
        value: Cow<'v, Value>,
        ctx: &Ctx<'_>,
    ) -> Checked<'v> {
        match rule.check(&value, ctx) {
            Outcome::Valid => Ok(value),
            Outcome::Value(output) => Ok(Cow::Owned(output)),
            Outcome::Error(error) => Err(error),
        }
    }

    fn format_error() -> ErrorTree {
        ErrorTree::code("FORMAT_ERROR")
    }

    /// A field of borrowed data, read into a slot; null when it is missing.
    #[inline]
    #[must_use]
    pub fn slot(value: Option<&Value>) -> Cow<'_, Value> {
        Cow::Borrowed(value.unwrap_or(&NULL))
    }

    /// A field taken from owned data, read into a slot; a borrowed null when
    /// it is missing.
    #[inline]
    pub fn take_slot(slot: &mut Option<Value>) -> Cow<'static, Value> {
        slot.take().map_or(Cow::Borrowed(&NULL), Cow::Owned)
    }

    /// The errors of the fields of a typed object.
    #[derive(Debug, Default)]
    pub struct Fields {
        errors: Option<BTreeMap<String, ErrorTree>>,
    }

    impl Fields {
        /// The value of a field, `None` after an error.
        #[inline]
        pub fn take<T>(&mut self, key: &str, result: Result<T, ErrorTree>) -> Option<T> {
            match result {
                Ok(value) => Some(value),
                Err(error) => {
                    self.errors
                        .get_or_insert_default()
                        .insert(key.to_owned(), error);
                    None
                }
            }
        }

        /// The errors, if any.
        ///
        /// # Errors
        ///
        /// The errors of the fields.
        #[inline]
        pub fn finish(self) -> Result<(), BTreeMap<String, ErrorTree>> {
            self.errors.map_or(Ok(()), Err)
        }
    }

    /// A required value: present by the rules.
    ///
    /// # Errors
    ///
    /// `FORMAT_ERROR` when the value is empty.
    #[inline]
    pub fn required<T>(value: Option<T>) -> Result<T, ErrorTree> {
        value.ok_or_else(format_error)
    }

    #[inline]
    fn convert<T: FromLivr + FromLivrRef>(value: Cow<'_, Value>) -> Result<T, ErrorTree> {
        match value {
            Cow::Borrowed(value) => T::from_livr_ref(value),
            Cow::Owned(value) => T::from_livr(value),
        }
        .map_err(|_| format_error())
    }

    /// A field checked by its rules, converted.
    ///
    /// # Errors
    ///
    /// The error of the rules.
    #[inline]
    pub fn field<T: FromLivr + FromLivrRef>(value: Checked<'_>) -> Result<T, ErrorTree> {
        convert(value?)
    }

    #[inline]
    fn object<T: TypedObject>(value: Cow<'_, Value>) -> Result<Option<T>, ErrorTree> {
        match value {
            value if is_no_value(&value) => Ok(None),
            Cow::Borrowed(Value::Object(fields)) => T::validate_fields(fields)
                .map(Some)
                .map_err(ErrorTree::Object),
            Cow::Owned(Value::Object(fields)) => T::validate_fields_owned(fields)
                .map(Some)
                .map_err(ErrorTree::Object),
            _ => Err(format_error()),
        }
    }

    /// A field checked by its rules, then validated by `nested_object`.
    ///
    /// # Errors
    ///
    /// The errors of the rules.
    #[inline]
    pub fn nested<T: TypedObject>(value: Checked<'_>) -> Result<Option<T>, ErrorTree> {
        object(value?)
    }

    fn items<T>(
        value: Cow<'_, Value>,
        mut item: impl FnMut(Cow<'_, Value>) -> Result<T, ErrorTree>,
    ) -> Result<Option<Vec<T>>, ErrorTree> {
        match value {
            value if is_no_value(&value) => Ok(None),
            Cow::Borrowed(Value::Array(list)) => collect(
                list.len(),
                list.iter().map(|value| item(Cow::Borrowed(value))),
            ),
            Cow::Owned(Value::Array(list)) => {
                let length = list.len();
                collect(
                    length,
                    list.into_iter().map(|value| item(Cow::Owned(value))),
                )
            }
            _ => Err(format_error()),
        }
    }

    // The items, or the errors of the items (`None` for valid ones)
    fn collect<T>(
        length: usize,
        results: impl Iterator<Item = Result<T, ErrorTree>>,
    ) -> Result<Option<Vec<T>>, ErrorTree> {
        let mut output = Vec::with_capacity(length);
        let mut errors: Option<Vec<Option<ErrorTree>>> = None;
        for (i, result) in results.enumerate() {
            match result {
                Ok(item) => {
                    if errors.is_none() {
                        output.push(item);
                    }
                }
                Err(error) => errors.get_or_insert_with(|| vec![None; length])[i] = Some(error),
            }
        }
        match errors {
            Some(errors) => Err(ErrorTree::List(errors)),
            None => Ok(Some(output)),
        }
    }

    /// A field checked by its rules, then validated by `list_of_objects`.
    ///
    /// # Errors
    ///
    /// The errors of the rules.
    pub fn list_of_objects<T: TypedObject>(
        value: Checked<'_>,
    ) -> Result<Option<Vec<T>>, ErrorTree> {
        items(value?, |item| object(item)?.ok_or_else(format_error))
    }

    /// A field checked by its rules, then validated by `list_of` with the
    /// rules of the items (`reads_ctx`: whether they read the item as a
    /// field).
    ///
    /// # Errors
    ///
    /// The errors of the rules.
    pub fn list_of<T: FromLivr + FromLivrRef>(
        value: Checked<'_>,
        reads_ctx: bool,
        rules: impl for<'a> Fn(Cow<'a, Value>, &Ctx<'_>) -> Checked<'a>,
    ) -> Result<Option<Vec<T>>, ErrorTree> {
        items(value?, |item| convert(single(&rules, reads_ctx, item)?))
    }

    // Runs rules on a value validated on its own, like `Chain::run_single`
    #[inline]
    fn single<'v>(
        rules: &impl for<'a> Fn(Cow<'a, Value>, &Ctx<'_>) -> Checked<'a>,
        reads_ctx: bool,
        value: Cow<'v, Value>,
    ) -> Checked<'v> {
        match value {
            Cow::Borrowed(value) => rules(Cow::Borrowed(value), &Ctx::single("field", value)),
            // The rules read the value as a field: they get it borrowed
            Cow::Owned(value) if reads_ctx => {
                let output = match rules(Cow::Borrowed(&value), &Ctx::single("field", &value))? {
                    Cow::Borrowed(_) => None,
                    Cow::Owned(output) => Some(output),
                };
                Ok(Cow::Owned(output.unwrap_or(value)))
            }
            value => rules(value, &Ctx::none()),
        }
    }
}
