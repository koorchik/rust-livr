//! Rules by name: the built-in ones, custom ones and aliases.

use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Arc, OnceLock};

use serde_json::Value;

use crate::compile::CompileCtx;
use crate::error::{Code, SchemaError};
use crate::rule::RuleBuilder;
use crate::rules::{BUILT_IN_RULES, CompiledRule};
use crate::util::camelize;

pub(crate) type BuildFn = fn(&[Value], &mut CompileCtx<'_>) -> Result<CompiledRule, SchemaError>;

#[derive(Clone)]
pub(crate) enum RuleDef {
    Builtin(BuildFn),
    Custom(Arc<dyn RuleBuilder>),
    Alias(Arc<AliasDef>),
}

pub(crate) struct AliasDef {
    pub(crate) name: Arc<str>,
    pub(crate) rules: Value,
    pub(crate) error: Option<Code>,
}

/// An alias: a name for a list of rules, with an optional error code that
/// replaces their errors.
///
/// Built with [`Alias::new`] and [`Alias::with_error`], or read from JSON with
/// [`Alias::from_json`]; the fields are private, so new options can come
/// without breaking code.
///
/// ```
/// use livr_core::Alias;
/// use serde_json::json;
///
/// let alias = Alias::new("adult_age", json!(["positive_integer", {"min_number": 18}]))
///     .with_error("WRONG_AGE");
/// assert_eq!(alias.name(), "adult_age");
/// assert_eq!(alias.error(), Some("WRONG_AGE"));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Alias {
    name: String,
    rules: Value,
    error: Option<String>,
}

impl Alias {
    /// An alias without its own error.
    pub fn new(name: impl Into<String>, rules: Value) -> Self {
        Self {
            name: name.into(),
            rules,
            error: None,
        }
    }

    /// The alias with its own error code.
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// The name of the alias, used as a rule name in schemas.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The rules of the alias: a rule, or a list of rules.
    #[must_use]
    pub fn rules(&self) -> &Value {
        &self.rules
    }

    /// The error code that replaces the errors of the rules, if any.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// An alias from JSON: `{"name": ..., "rules": ..., "error": ...}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the name or the rules are missing, or the error
    /// is not a string.
    pub fn from_json(json: &Value) -> Result<Self, SchemaError> {
        let name = match json.get("name") {
            Some(Value::String(name)) if !name.is_empty() => name.clone(),
            _ => {
                return Err(SchemaError::new(format!(
                    "an alias needs a name, got {json}"
                )));
            }
        };
        let Some(rules) = json.get("rules") else {
            return Err(SchemaError::new(format!("alias [{name}] has no rules")));
        };
        let error = match json.get("error") {
            None | Some(Value::Null) => None,
            Some(Value::String(error)) => Some(error.clone()),
            Some(_) => {
                return Err(SchemaError::new(format!(
                    "the error of alias [{name}] must be a string"
                )));
            }
        };
        Ok(Self {
            name,
            rules: rules.clone(),
            error,
        })
    }
}

/// Rules by name. [`Registry::builtin`] has the rules of the LIVR
/// specification; a copy of it (`Registry::default()`) can get custom rules
/// and aliases. Every rule is also registered with its camelCase name.
///
/// ```
/// use livr_core::{Alias, Registry, Validator};
/// use serde_json::json;
/// let mut registry = Registry::default();
/// registry.register_alias(Alias::new("adult_age", json!(["positive_integer", {"min_number": 18}])))?;
/// let validator = Validator::builder(&json!({"age": "adult_age"})).registry(&registry).build()?;
/// assert!(validator.validate(&json!({"age": 12})).is_err());
/// # Ok::<(), livr_core::SchemaError>(())
/// ```
#[derive(Clone)]
pub struct Registry {
    rules: HashMap<Arc<str>, RuleDef, BuildHasherDefault<NameHasher>>,
    // camelCase names registered for snake_case ones: camelCase -> snake_case
    camelized: HashMap<Arc<str>, Arc<str>>,
}

// FNV-1a: rule names are short and come from schemas, not from data, so a
// fast hash matters more than resistance to collisions (a validator built for
// every validation looks up all its rules)
#[derive(Default)]
struct NameHasher(u64);

impl Hasher for NameHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for &byte in bytes {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl Registry {
    /// The built-in rules.
    #[must_use]
    pub fn builtin() -> &'static Self {
        static BUILTIN: OnceLock<Registry> = OnceLock::new();
        BUILTIN.get_or_init(|| {
            let mut registry = Self::empty();
            for &(name, build) in BUILT_IN_RULES {
                registry.add(name, RuleDef::Builtin(build));
            }
            registry
        })
    }

    /// A registry without rules.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            rules: HashMap::default(),
            camelized: HashMap::new(),
        }
    }

    /// Registers a custom rule, replacing a rule with the same name.
    pub fn register_rule(&mut self, name: &str, builder: impl RuleBuilder + 'static) -> &mut Self {
        self.add(name, RuleDef::Custom(Arc::new(builder)));
        self
    }

    /// Registers an alias. Its rules are compiled with the validator, so an
    /// alias can use rules and aliases registered after it.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the name is empty.
    pub fn register_alias(&mut self, alias: Alias) -> Result<&mut Self, SchemaError> {
        if alias.name.is_empty() {
            return Err(SchemaError::new("an alias needs a name"));
        }
        let def = AliasDef {
            name: alias.name.as_str().into(),
            rules: alias.rules,
            error: alias.error.map(Code::from),
        };
        self.add(&alias.name, RuleDef::Alias(Arc::new(def)));
        Ok(self)
    }

    /// Whether a rule is registered with this name.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.rules.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&RuleDef> {
        self.rules.get(name)
    }

    fn add(&mut self, name: &str, def: RuleDef) {
        let name: Arc<str> = name.into();
        // A name registered on its own is no longer the camelCase form of
        // another one
        self.camelized.remove(&name);
        let camelized = camelize(&name);
        if *camelized != *name {
            let camelized: Arc<str> = camelized.into();
            let is_own_rule =
                self.rules.contains_key(&camelized) && !self.camelized.contains_key(&camelized);
            if !is_own_rule {
                self.rules.insert(camelized.clone(), def.clone());
                self.camelized.insert(camelized, name.clone());
            }
        }
        self.rules.insert(name, def);
    }
}

impl Default for Registry {
    /// A copy of the built-in rules.
    fn default() -> Self {
        Self::builtin().clone()
    }
}

// Like `Validator`: custom rule builders are `Arc<dyn RuleBuilder>`, not
// unwind safe by their type, but a registry is only changed through
// `&mut self`, and builders are called through `&self`.
impl std::panic::UnwindSafe for Registry {}
impl std::panic::RefUnwindSafe for Registry {}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<_> = self.rules.keys().collect();
        names.sort();
        f.debug_struct("Registry").field("rules", &names).finish()
    }
}
