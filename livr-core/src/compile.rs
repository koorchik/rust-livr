//! Compiling a schema into rules: field chains, objects, aliases.
//!
//! Compiled rules run on a `Cow<Value>`: a borrowed value stays borrowed
//! until a rule changes it, an owned one is moved through the rules, so the
//! same code validates borrowed data (copying only what goes to the output)
//! and owned data (copying nothing).

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, LazyLock};

use serde_json::{Map, Value};

use crate::error::{Code, ErrorTree, SchemaError};
use crate::registry::{AliasDef, Registry, RuleDef};
use crate::rule::{Ctx, Outcome};
use crate::rules::CompiledRule;

static NULL: Value = Value::Null;

// Whether objects iterate in key order: `serde_json::Map` is a `BTreeMap`
// unless the `preserve_order` feature of serde_json is on, which any crate of
// the build can turn on
static MAP_IS_SORTED: LazyLock<bool> = LazyLock::new(|| {
    let map: Map<String, Value> = [("b", Value::Null), ("a", Value::Null)]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    map.keys().next().is_some_and(|key| key == "a")
});

/// The result of rules on a value: the output, borrowed when the rules did
/// not change the value, or the errors.
pub type Checked<'v> = Result<Cow<'v, Value>, ErrorTree>;

/// The result of rules from the outcome of a rule.
#[inline]
pub(crate) fn checked(outcome: Outcome, value: Cow<'_, Value>) -> Checked<'_> {
    match outcome {
        Outcome::Valid => Ok(value),
        Outcome::Value(output) => Ok(Cow::Owned(output)),
        Outcome::Error(error) => Err(error),
    }
}

/// The outcome of rules on a borrowed value.
#[inline]
pub(crate) fn outcome(checked: Checked<'_>) -> Outcome {
    match checked {
        Ok(Cow::Borrowed(_)) => Outcome::Valid,
        Ok(Cow::Owned(output)) => Outcome::Value(output),
        Err(error) => Outcome::Error(error),
    }
}

/// Runs rules on the items of a list (the value must be a list). For a
/// borrowed list, a new list is built only when an item changes.
pub(crate) fn run_items<'v>(
    list: Cow<'v, Value>,
    mut run: impl FnMut(Cow<'v, Value>) -> Checked<'v>,
) -> Checked<'v> {
    let mut errors: Option<Vec<Option<ErrorTree>>> = None;
    match list {
        Cow::Borrowed(list @ Value::Array(items)) => {
            let mut output: Option<Vec<Value>> = None;
            for (i, item) in items.iter().enumerate() {
                match run(Cow::Borrowed(item)) {
                    Ok(Cow::Borrowed(_)) => {
                        if let Some(output) = &mut output {
                            output.push(item.clone());
                        }
                    }
                    Ok(Cow::Owned(new_item)) => {
                        if errors.is_none() {
                            output
                                .get_or_insert_with(|| {
                                    let mut output = Vec::with_capacity(items.len());
                                    output.extend_from_slice(&items[..i]);
                                    output
                                })
                                .push(new_item);
                        }
                    }
                    Err(error) => {
                        errors.get_or_insert_with(|| vec![None; items.len()])[i] = Some(error);
                        output = None;
                    }
                }
            }
            match (errors, output) {
                (Some(errors), _) => Err(ErrorTree::List(errors)),
                (None, Some(output)) => Ok(Cow::Owned(Value::Array(output))),
                (None, None) => Ok(Cow::Borrowed(list)),
            }
        }
        // The items are validated in place: no new list
        Cow::Owned(Value::Array(mut items)) => {
            let length = items.len();
            for (i, item) in items.iter_mut().enumerate() {
                match run(Cow::Owned(std::mem::take(item))) {
                    Ok(output) => *item = output.into_owned(),
                    Err(error) => errors.get_or_insert_with(|| vec![None; length])[i] = Some(error),
                }
            }
            match errors {
                Some(errors) => Err(ErrorTree::List(errors)),
                None => Ok(Cow::Owned(Value::Array(items))),
            }
        }
        _ => Err(ErrorTree::code("FORMAT_ERROR")),
    }
}

/// How deep rules can nest: metarules (`nested_object`, `list_of`, `or`, ...)
/// and aliases each add a level. A schema or a registry can come from
/// untrusted storage, and a deeper one would overflow the stack while it is
/// compiled; real schemas are a few levels deep.
pub const MAX_DEPTH: usize = 64;

/// How many rules a schema can compile to, aliases expanded. Aliases that use
/// another alias twice double at every level, so a small registry could
/// otherwise take minutes and gigabytes to compile.
pub const MAX_RULES: usize = 100_000;

/// Compiles rules for rule builders, so metarules compile nested rules with
/// the rules of the validator, the custom ones and aliases included.
///
/// A schema is limited to [`MAX_DEPTH`] levels of nested rules and to
/// [`MAX_RULES`] compiled rules; beyond them it is a [`SchemaError`].
pub struct CompileCtx<'r> {
    registry: RegistryRef<'r>,
    // Aliases being compiled, to report recursive ones
    aliases: Vec<Arc<str>>,
    // Rules being compiled, nested in each other
    depth: usize,
    // Rules compiled so far, nested ones and aliases included
    rule_count: usize,
}

impl fmt::Debug for CompileCtx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompileCtx")
            .field("aliases", &self.aliases)
            .field("depth", &self.depth)
            .field("rule_count", &self.rule_count)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
enum RegistryRef<'r> {
    Ready(&'r Registry),
    // Got when a rule is compiled: rules that compile no other rules never
    // need it
    Lazy(fn() -> &'static Registry),
}

impl<'r> CompileCtx<'r> {
    /// A context with the rules of the registry.
    #[must_use]
    pub fn new(registry: &'r Registry) -> Self {
        Self {
            registry: RegistryRef::Ready(registry),
            aliases: Vec::new(),
            depth: 0,
            rule_count: 0,
        }
    }

    /// A context with the rules of a registry got on first use: only when a
    /// rule compiles other rules (metarules, `or`). For code that builds
    /// rules one by one, like the code of `schema!`, where building the
    /// registry would be most of a cold start.
    ///
    /// Not part of the public API: for the code generated by `schema!`.
    #[doc(hidden)]
    #[must_use]
    pub fn lazy(registry: fn() -> &'static Registry) -> CompileCtx<'static> {
        CompileCtx {
            registry: RegistryRef::Lazy(registry),
            aliases: Vec::new(),
            depth: 0,
            rule_count: 0,
        }
    }

    /// Compiles the rules of a field: a rule, or a list of rules.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for an invalid rule.
    pub fn compile_field(&mut self, rules: &Value) -> Result<Chain, SchemaError> {
        let mut compiled = Vec::with_capacity(rule_count(rules));
        self.compile_rules(rules, &mut compiled)?;
        let rules = compiled.into_boxed_slice();
        let reads_ctx = rules.iter().any(CompiledRule::reads_ctx);
        Ok(Chain { rules, reads_ctx })
    }

    /// Compiles a schema: an object of field rules.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the schema is not an object or has an invalid
    /// rule.
    pub fn compile_object(&mut self, schema: &Value) -> Result<ObjectValidator, SchemaError> {
        let Value::Object(schema) = schema else {
            return Err(SchemaError::new(format!(
                "a schema must be an object, got {schema}"
            )));
        };
        // The names and the rules of all fields in one allocation each, so a
        // validator built for every validation is cheap
        let mut names = String::with_capacity(schema.keys().map(String::len).sum());
        let mut rules = Vec::with_capacity(schema.values().map(rule_count).sum());
        let mut fields = Vec::with_capacity(schema.len());
        for (name, field_rules) in schema {
            // A field with an empty list of rules is not validated and not in
            // the output, like a field that is not in the schema
            if field_rules.as_array().is_some_and(Vec::is_empty) {
                continue;
            }
            let name_start = names.len();
            names.push_str(name);
            let rules_start = rules.len();
            self.compile_rules(field_rules, &mut rules)?;
            fields.push(Field {
                name: (name_start, names.len()),
                rules: (rules_start, rules.len()),
            });
        }
        let names = names.into_boxed_str();
        let name = |field: &Field| &names[field.name.0..field.name.1];
        let is_sorted = fields.is_sorted_by(|a, b| name(a) < name(b));
        Ok(ObjectValidator {
            reads_siblings: rules.iter().any(CompiledRule::reads_ctx),
            is_sorted,
            names,
            fields: fields.into_boxed_slice(),
            rules: rules.into_boxed_slice(),
        })
    }

    fn compile_rules(
        &mut self,
        rules: &Value,
        compiled: &mut Vec<CompiledRule>,
    ) -> Result<(), SchemaError> {
        match rules {
            Value::Array(rules) => {
                for rule in rules {
                    compiled.push(self.compile_rule(rule)?);
                }
            }
            rule => compiled.push(self.compile_rule(rule)?),
        }
        Ok(())
    }

    pub(crate) fn compile_rule(&mut self, rule: &Value) -> Result<CompiledRule, SchemaError> {
        // Checked before the rule is built, which can compile nested rules
        if self.depth >= MAX_DEPTH {
            return Err(SchemaError::new(format!(
                "rules are nested more than {MAX_DEPTH} levels deep"
            )));
        }
        self.rule_count += 1;
        if self.rule_count > MAX_RULES {
            return Err(SchemaError::new(format!(
                "the schema has more than {MAX_RULES} rules, aliases expanded"
            )));
        }
        self.depth += 1;
        let compiled = self.compile_rule_at_depth(rule);
        self.depth -= 1;
        compiled
    }

    fn compile_rule_at_depth(&mut self, rule: &Value) -> Result<CompiledRule, SchemaError> {
        let (name, args): (&str, &[Value]) = match rule {
            Value::String(name) => (name, &[]),
            Value::Object(rule) if rule.len() == 1 => {
                let (name, args) = rule.iter().next().expect("one entry");
                let args = match args {
                    Value::Array(args) => args.as_slice(),
                    arg => std::slice::from_ref(arg),
                };
                (name, args)
            }
            Value::Object(_) => {
                return Err(SchemaError::new(format!(
                    "a rule object must have exactly one rule name, got {rule}"
                )));
            }
            _ => {
                return Err(SchemaError::new(format!(
                    "a rule must be a name or an object with arguments, got {rule}"
                )));
            }
        };

        let registry = match self.registry {
            RegistryRef::Ready(registry) => registry,
            RegistryRef::Lazy(registry) => {
                let registry = registry();
                self.registry = RegistryRef::Ready(registry);
                registry
            }
        };
        match registry.get(name) {
            Some(RuleDef::Builtin(build)) => build(args, self),
            Some(RuleDef::Custom(builder)) => Ok(CompiledRule::Custom(builder.build(args, self)?)),
            Some(RuleDef::Alias(alias)) => self.compile_alias(alias),
            None => Err(SchemaError::new(format!("rule [{name}] not registered"))),
        }
    }

    fn compile_alias(&mut self, alias: &AliasDef) -> Result<CompiledRule, SchemaError> {
        if self.aliases.iter().any(|name| **name == *alias.name) {
            return Err(SchemaError::new(format!(
                "alias [{}] is recursive",
                alias.name
            )));
        }
        self.aliases.push(alias.name.clone());
        let chain = self.compile_field(&alias.rules);
        self.aliases.pop();
        Ok(CompiledRule::Alias(AliasRule {
            chain: chain?,
            error: alias.error.clone(),
        }))
    }
}

fn rule_count(rules: &Value) -> usize {
    rules.as_array().map_or(1, Vec::len)
}

// Each rule gets the output of the previous one, the first error stops
#[inline]
fn run_rules<'v>(rules: &[CompiledRule], mut value: Cow<'v, Value>, ctx: &Ctx<'_>) -> Checked<'v> {
    for rule in rules {
        value = rule.run(value, ctx)?;
    }
    Ok(value)
}

/// The compiled rules of a field: each rule gets the output of the previous
/// one, the first error stops the chain.
#[derive(Debug)]
pub struct Chain {
    rules: Box<[CompiledRule]>,
    // Whether a rule reads other fields: then the value cannot be moved into
    // the rules while they read it
    reads_ctx: bool,
}

impl Chain {
    /// Checks the value with the rules.
    #[inline]
    pub fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        outcome(self.run(Cow::Borrowed(value), ctx))
    }

    /// Runs the rules: a borrowed value stays borrowed unless a rule changes
    /// it, an owned one is moved through the rules. For an owned value, the
    /// context has the other fields only when [`reads_ctx`](Self::reads_ctx).
    #[inline]
    pub fn run<'v>(&self, value: Cow<'v, Value>, ctx: &Ctx<'_>) -> Checked<'v> {
        run_rules(&self.rules, value, ctx)
    }

    /// Runs the rules on a value validated on its own, as the field `name`
    /// (list items, `or`, aliases).
    #[inline]
    pub fn run_single<'v>(&self, name: &str, value: Cow<'v, Value>) -> Checked<'v> {
        match value {
            Cow::Borrowed(value) => self.run(Cow::Borrowed(value), &Ctx::single(name, value)),
            // The rules read the value as a field: they get it borrowed
            Cow::Owned(value) if self.reads_ctx => {
                let outcome = self.check(&value, &Ctx::single(name, &value));
                checked(outcome, Cow::Owned(value))
            }
            value => self.run(value, &Ctx::none()),
        }
    }

    /// Whether a rule reads other fields of the object (`equal_to_field`,
    /// custom rules).
    #[inline]
    #[must_use]
    pub fn reads_ctx(&self) -> bool {
        self.reads_ctx
    }
}

struct Field {
    // Ranges in ObjectValidator::names and ObjectValidator::rules
    name: (usize, usize),
    rules: (usize, usize),
}

/// A compiled schema: validates an object, field by field.
pub struct ObjectValidator {
    names: Box<str>,
    fields: Box<[Field]>,
    rules: Box<[CompiledRule]>,
    // Whether a rule reads other fields: then the fields cannot be moved out
    // of the object while they are validated
    reads_siblings: bool,
    // Whether the fields are sorted by name, to find them by binary search
    is_sorted: bool,
}

impl fmt::Debug for ObjectValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = self
            .fields
            .iter()
            .map(|field| (self.name(field), self.rules(field)));
        f.debug_struct("ObjectValidator")
            .field("fields", &DebugMap(fields))
            .field("reads_siblings", &self.reads_siblings)
            .finish_non_exhaustive()
    }
}

// The fields of an object as a map, by name
struct DebugMap<I>(I);

impl<'a, I> fmt::Debug for DebugMap<I>
where
    I: Iterator<Item = (&'a str, &'a [CompiledRule])> + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.0.clone()).finish()
    }
}

impl ObjectValidator {
    #[inline]
    fn name(&self, field: &Field) -> &str {
        &self.names[field.name.0..field.name.1]
    }

    #[inline]
    fn rules(&self, field: &Field) -> &[CompiledRule] {
        &self.rules[field.rules.0..field.rules.1]
    }

    /// Checks the value: `FORMAT_ERROR` unless it is an object, the errors of
    /// the fields, or the output with the fields of the schema only.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        outcome(self.run(Cow::Borrowed(value)))
    }

    /// Runs the schema on a value: the output is a new object.
    #[inline]
    pub fn run<'v>(&self, value: Cow<'v, Value>) -> Checked<'v> {
        let result = match value {
            Cow::Borrowed(Value::Object(data)) => self.validate_map(data),
            Cow::Owned(Value::Object(data)) => self.validate_map_owned(data),
            _ => return Err(ErrorTree::code("FORMAT_ERROR")),
        };
        result
            .map(|output| Cow::Owned(Value::Object(output)))
            .map_err(ErrorTree::Object)
    }

    /// Validates the fields of an object.
    ///
    /// # Errors
    ///
    /// The errors of the invalid fields.
    pub fn validate_map(
        &self,
        data: &Map<String, Value>,
    ) -> Result<Map<String, Value>, BTreeMap<String, ErrorTree>> {
        let ctx = Ctx::object(data);
        let mut output = Map::new();
        let mut errors: Option<BTreeMap<String, ErrorTree>> = None;
        // When both the fields and the data are in key order, one pass over
        // both instead of a lookup per field, unless the data has many more
        // fields than the schema
        let mut entries =
            (self.is_sorted && data.len() <= 2 * self.fields.len() + 8 && *MAP_IS_SORTED)
                .then(|| data.iter().peekable());
        for field in &self.fields {
            let name = self.name(field);
            // A missing field is null for the rules
            let value = match &mut entries {
                Some(entries) => loop {
                    match entries.peek().map(|(key, _)| key.as_str().cmp(name)) {
                        Some(Ordering::Less) => {
                            entries.next();
                        }
                        Some(Ordering::Equal) => break entries.next().map(|(_, value)| value),
                        Some(Ordering::Greater) | None => break None,
                    }
                },
                None => data.get(name),
            };
            match run_rules(
                self.rules(field),
                Cow::Borrowed(value.unwrap_or(&NULL)),
                &ctx,
            ) {
                // A missing field stays missing, a null value stays null
                Ok(Cow::Borrowed(output_value)) => {
                    if value.is_some() && errors.is_none() {
                        output.insert(name.to_owned(), output_value.clone());
                    }
                }
                Ok(Cow::Owned(output_value)) => {
                    if errors.is_none() {
                        output.insert(name.to_owned(), output_value);
                    }
                }
                Err(error) => {
                    errors
                        .get_or_insert_default()
                        .insert(name.to_owned(), error);
                }
            }
        }
        match errors {
            None => Ok(output),
            Some(errors) => Err(errors),
        }
    }

    /// Validates the fields of an owned object in place: unchanged values
    /// stay where they are, fields not in the schema are removed.
    ///
    /// # Errors
    ///
    /// The errors of the invalid fields.
    pub fn validate_map_owned(
        &self,
        mut data: Map<String, Value>,
    ) -> Result<Map<String, Value>, BTreeMap<String, ErrorTree>> {
        if self.reads_siblings {
            return self.validate_map(&data);
        }
        let ctx = Ctx::none();
        let mut errors: Option<BTreeMap<String, ErrorTree>> = None;
        // Fields of the data that are in the schema, to know whether there
        // are others to remove
        let mut found = 0;
        for field in &self.fields {
            let name = self.name(field);
            let rules = self.rules(field);
            let result = match data.get_mut(name) {
                Some(slot) => match run_rules(rules, Cow::Owned(std::mem::take(slot)), &ctx) {
                    Ok(output) => {
                        *slot = output.into_owned();
                        found += 1;
                        continue;
                    }
                    Err(error) => Err(error),
                },
                // Borrowed: an owned output is a value given by a rule
                None => run_rules(rules, Cow::Borrowed(&NULL), &ctx),
            };
            match result {
                // A missing field stays missing unless a rule gives it a value
                Ok(Cow::Owned(output)) => {
                    data.insert(name.to_owned(), output);
                    found += 1;
                }
                Ok(Cow::Borrowed(_)) => {}
                Err(error) => {
                    errors
                        .get_or_insert_default()
                        .insert(name.to_owned(), error);
                }
            }
        }
        if let Some(errors) = errors {
            return Err(errors);
        }
        if data.len() > found {
            data.retain(|key, _| self.has_field(key));
        }
        Ok(data)
    }

    #[inline]
    fn has_field(&self, name: &str) -> bool {
        if self.is_sorted {
            self.fields
                .binary_search_by(|field| self.name(field).cmp(name))
                .is_ok()
        } else {
            self.fields.iter().any(|field| self.name(field) == name)
        }
    }
}

/// An alias: its rules see the value as the only field `value`, its error
/// replaces the errors of the rules.
#[derive(Debug)]
pub(crate) struct AliasRule {
    chain: Chain,
    error: Option<Code>,
}

impl AliasRule {
    #[inline]
    pub(crate) fn run<'v>(&self, value: Cow<'v, Value>) -> Checked<'v> {
        let result = self.chain.run_single("value", value);
        match (result, &self.error) {
            (Err(_), Some(error)) => Err(ErrorTree::Code(error.clone())),
            (result, _) => result,
        }
    }
}
