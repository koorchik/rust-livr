//! Inference of the type of the output from the rules, like the TypeScript
//! types of js-validator-livr: the rules of a field are folded into a state,
//! its type and whether it is present (not empty). Types are sound: for every
//! valid data, the output has the type (`FromLivr` never fails for built-in
//! rules).

use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use serde_json::{Map, Value};
use syn::Ident;

use crate::names::{pascal_case, variant_name};
use crate::parse::CustomRule;

// At compile time: size does not matter
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum Ty {
    // Any value: rules that tell nothing about the type
    Value,
    String,
    I64,
    F64,
    Bool,
    Map,
    // A list, and whether its items are present
    List(Box<Ty>, bool),
    Named(Ident),
    Custom(syn::Type),
}

impl PartialEq for Ty {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value, Self::Value)
            | (Self::String, Self::String)
            | (Self::I64, Self::I64)
            | (Self::F64, Self::F64)
            | (Self::Bool, Self::Bool)
            | (Self::Map, Self::Map) => true,
            (Self::List(a, a_present), Self::List(b, b_present)) => {
                a == b && a_present == b_present
            }
            (Self::Named(a), Self::Named(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => {
                quote::quote!(#a).to_string() == quote::quote!(#b).to_string()
            }
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct State {
    pub ty: Ty,
    pub present: bool,
}

impl State {
    const UNKNOWN: Self = Self {
        ty: Ty::Value,
        present: false,
    };
}

pub struct FieldDef {
    pub key: String,
    pub rules: Value,
    pub state: State,
}

pub enum TypeDef {
    Struct {
        name: Ident,
        fields: Vec<FieldDef>,
    },
    // `one_of` strings
    StrEnum {
        name: Ident,
        values: Vec<String>,
    },
    // `variable_object`: variants by the value of the selector field
    Tagged {
        name: Ident,
        selector: String,
        variants: Vec<(String, Ident)>,
    },
    // `or` with rule sets of different types
    Union {
        name: Ident,
        variants: Vec<Ty>,
    },
}

pub struct Infer<'a> {
    pub defs: Vec<TypeDef>,
    names: HashSet<String>,
    custom: &'a [CustomRule],
    aliases: &'a HashMap<String, Value>,
    alias_stack: Vec<String>,
    // The name given with `as Name` to the first type generated for a field
    next_name: Option<String>,
}

// Rules that keep the value as it is (or as a string equal to it), for the
// selector field of `variable_object`
const SELECTOR_RULES: &[&str] = &[
    "required",
    "not_empty",
    "string",
    "eq",
    "one_of",
    "like",
    "min_length",
    "max_length",
    "length_between",
    "length_equal",
    "email",
    "url",
    "iso_date",
    "equal_to_field",
];

const BUILT_IN_RULES: &[&str] = &[
    "required",
    "not_empty",
    "not_empty_list",
    "any_object",
    "string",
    "eq",
    "one_of",
    "max_length",
    "min_length",
    "length_between",
    "length_equal",
    "like",
    "integer",
    "positive_integer",
    "decimal",
    "positive_decimal",
    "max_number",
    "min_number",
    "number_between",
    "email",
    "url",
    "iso_date",
    "equal_to_field",
    "nested_object",
    "variable_object",
    "list_of",
    "list_of_objects",
    "list_of_different_objects",
    "or",
    "trim",
    "to_lc",
    "to_uc",
    "remove",
    "leave_only",
    "default",
];

enum Resolved<'a> {
    Builtin(&'static str),
    Alias(&'a str, &'a Value),
    Custom(&'a CustomRule),
}

pub fn rule_parts(rule: &Value) -> Option<(&str, &[Value])> {
    match rule {
        Value::String(name) => Some((name, &[])),
        Value::Object(rule) if rule.len() == 1 => {
            let (name, args) = rule.iter().next()?;
            let args = match args {
                Value::Array(args) => args.as_slice(),
                arg => std::slice::from_ref(arg),
            };
            Some((name, args))
        }
        _ => None,
    }
}

pub fn rule_list(rules: &Value) -> &[Value] {
    match rules {
        Value::Array(rules) => rules,
        rule => std::slice::from_ref(rule),
    }
}

fn literal_ty(value: &Value) -> Ty {
    match value {
        Value::String(_) => Ty::String,
        Value::Number(number) if number.is_i64() => Ty::I64,
        Value::Number(_) => Ty::F64,
        Value::Bool(_) => Ty::Bool,
        _ => Ty::Value,
    }
}

impl<'a> Infer<'a> {
    pub fn new(custom: &'a [CustomRule], aliases: &'a HashMap<String, Value>) -> Self {
        Self {
            defs: Vec::new(),
            names: HashSet::new(),
            custom,
            aliases,
            alias_stack: Vec::new(),
            next_name: None,
        }
    }

    /// The name of the next generated type, from `as Name`; empty to clear.
    pub fn set_next_name(&mut self, name: String) {
        self.next_name = (!name.is_empty()).then_some(name);
    }

    /// The name of a generated type, for the variants of unions.
    pub fn short_name(&self, name: &Ident) -> String {
        name.to_string()
    }

    /// Takes a name given with `as Name`, so generated names avoid it.
    pub fn reserve(&mut self, name: String) {
        self.names.insert(name);
    }

    /// A name for a generated type, unique in the schema: the one of
    /// `as Name` (reserved), or the hint with a number when it is taken.
    pub fn unique_name(&mut self, name: &str) -> Ident {
        if let Some(name) = self.next_name.take() {
            return Ident::new(&name, Span::call_site());
        }
        let mut unique = name.to_owned();
        let mut i = 2;
        while !self.names.insert(unique.clone()) {
            unique = format!("{name}{i}");
            i += 1;
        }
        Ident::new(&unique, Span::call_site())
    }

    /// The built-in rule of a name, unless a custom rule or an alias has it.
    pub fn builtin_name(&self, name: &str) -> Option<&'static str> {
        match self.resolve(name)? {
            Resolved::Builtin(rule) => Some(rule),
            _ => None,
        }
    }

    /// Whether a custom rule has the name.
    pub fn is_custom(&self, name: &str) -> bool {
        matches!(self.resolve(name), Some(Resolved::Custom(_)))
    }

    /// Whether the type is a generated struct.
    pub fn is_struct(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Named(name) if self.defs.iter().any(|def| matches!(def, TypeDef::Struct { name: struct_name, .. } if struct_name == name)))
    }

    fn resolve(&self, name: &str) -> Option<Resolved<'a>> {
        let camelized = |rule: &str| livr_core::util::camelize(rule) == name;
        // Custom rules and aliases replace built-in rules with the same name
        if let Some(rule) = self
            .custom
            .iter()
            .rev()
            .find(|rule| rule.name == name || camelized(&rule.name))
        {
            return Some(Resolved::Custom(rule));
        }
        if let Some((alias, rules)) = self
            .aliases
            .iter()
            .find(|(alias, _)| *alias == name || camelized(alias))
        {
            return Some(Resolved::Alias(alias, rules));
        }
        BUILT_IN_RULES
            .iter()
            .find(|rule| **rule == name || camelized(rule))
            .map(|rule| Resolved::Builtin(rule))
    }

    /// The struct of a schema, with its fields in their order.
    pub fn object(
        &mut self,
        schema: &Map<String, Value>,
        name: Ident,
        skip: Option<&str>,
    ) -> Ident {
        let mut fields = Vec::new();
        for (key, rules) in schema {
            if Some(key.as_str()) == skip || rules.as_array().is_some_and(Vec::is_empty) {
                continue;
            }
            let hint = format!("{name}{}", pascal_case(key));
            let state = self.chain(rules, State::UNKNOWN, &hint);
            fields.push(FieldDef {
                key: key.clone(),
                rules: rules.clone(),
                state,
            });
        }
        self.defs.push(TypeDef::Struct {
            name: name.clone(),
            fields,
        });
        name
    }

    /// The state after the rules of a field.
    pub fn chain(&mut self, rules: &Value, mut state: State, hint: &str) -> State {
        for rule in rule_list(rules) {
            state = self.rule(rule, state, hint);
        }
        state
    }

    fn rule(&mut self, rule: &Value, mut state: State, hint: &str) -> State {
        // The schema is checked before: the rule is valid
        let Some((name, args)) = rule_parts(rule) else {
            return State::UNKNOWN;
        };
        let rule = match self.resolve(name) {
            Some(Resolved::Builtin(rule)) => rule,
            Some(Resolved::Alias(alias, rules)) => {
                if self.alias_stack.iter().any(|name| name == alias) {
                    return State::UNKNOWN;
                }
                self.alias_stack.push(alias.to_owned());
                let state = self.chain(rules, state, hint);
                self.alias_stack.pop();
                return state;
            }
            Some(Resolved::Custom(rule)) => {
                if let Some(output) = &rule.output {
                    state.ty = Ty::Custom(output.clone());
                }
                state.present |= rule.present;
                return state;
            }
            None => return State::UNKNOWN,
        };
        let arg = |i: usize| args.get(i).unwrap_or(&Value::Null);
        match rule {
            "required" => state.present = true,
            "not_empty" | "equal_to_field" => {}
            "not_empty_list" => {
                state.present = true;
                if !matches!(state.ty, Ty::List(..)) {
                    state.ty = Ty::List(Box::new(Ty::Value), true);
                }
            }
            "any_object" => state.ty = Ty::Map,
            // String rules output strings, a narrower type stays
            "string" | "email" | "url" | "iso_date" | "like" | "min_length" | "max_length"
            | "length_between" | "length_equal" => {
                if !self.is_str_enum(&state.ty) {
                    state.ty = Ty::String;
                }
            }
            "eq" => state.ty = literal_ty(arg(0)),
            "one_of" => {
                let values = match args {
                    [Value::Array(values)] => values.as_slice(),
                    values => values,
                };
                state.ty = self.one_of(values, hint);
            }
            "integer" | "positive_integer" => state.ty = Ty::I64,
            "decimal" | "positive_decimal" => state.ty = Ty::F64,
            "min_number" | "max_number" | "number_between" => {
                if !matches!(state.ty, Ty::I64 | Ty::F64) {
                    state.ty = Ty::F64;
                }
            }
            // Modifiers change primitive values only: values of an unknown
            // type may be lists or objects
            "to_lc" | "to_uc" => state.ty = self.modified(state.ty),
            // These can leave an empty string
            "trim" | "remove" | "leave_only" => {
                state.ty = self.modified(state.ty);
                state.present = false;
            }
            "default" => {
                let value = arg(0);
                if !livr_core::util::is_no_value(value) {
                    state.present = true;
                }
                let fits = match (&state.ty, literal_ty(value)) {
                    (Ty::Value, _) => true,
                    (Ty::F64, Ty::I64) => true,
                    (Ty::Named(name), Ty::String) => {
                        self.str_enum_values(name).is_some_and(|values| {
                            values.iter().any(|v| Some(v.as_str()) == value.as_str())
                        })
                    }
                    (ty, literal) => *ty == literal,
                };
                if !fits {
                    state.ty = Ty::Value;
                }
            }
            "nested_object" => {
                if let Value::Object(schema) = arg(0) {
                    let name = self.unique_name(hint);
                    state.ty = Ty::Named(self.object(schema, name, None));
                }
            }
            "list_of_objects" => {
                if let Value::Object(schema) = arg(0) {
                    let name = self.unique_name(&format!("{hint}Item"));
                    state.ty = Ty::List(Box::new(Ty::Named(self.object(schema, name, None))), true);
                }
            }
            "list_of" => {
                let item_rules = match args {
                    [rules @ Value::Array(_)] => rules.clone(),
                    rules => Value::Array(rules.to_vec()),
                };
                let item = self.chain(&item_rules, State::UNKNOWN, &format!("{hint}Item"));
                state.ty = Ty::List(Box::new(item.ty), item.present);
            }
            "variable_object" => state.ty = self.tagged(arg(0), arg(1), hint),
            "list_of_different_objects" => {
                state.ty = Ty::List(
                    Box::new(self.tagged(arg(0), arg(1), &format!("{hint}Item"))),
                    true,
                );
            }
            "or" => state = self.or(args, state, hint),
            _ => state = State::UNKNOWN,
        }
        state
    }

    fn str_enum_values(&self, name: &Ident) -> Option<&[String]> {
        self.defs.iter().find_map(|def| match def {
            TypeDef::StrEnum {
                name: enum_name,
                values,
            } if enum_name == name => Some(values.as_slice()),
            _ => None,
        })
    }

    fn is_str_enum(&self, ty: &Ty) -> bool {
        match ty {
            Ty::String => true,
            Ty::Named(name) => self.str_enum_values(name).is_some(),
            _ => false,
        }
    }

    // The type after a modifier: primitive values become strings, lists and
    // objects stay as they are
    fn modified(&self, ty: Ty) -> Ty {
        match ty {
            Ty::String | Ty::I64 | Ty::F64 | Ty::Bool => Ty::String,
            Ty::Map | Ty::List(..) => ty,
            Ty::Named(ref name) => {
                if self.str_enum_values(name).is_some() {
                    Ty::String
                } else if self
                    .defs
                    .iter()
                    .any(|def| matches!(def, TypeDef::Union { name: union, .. } if union == name))
                {
                    Ty::Value
                } else {
                    ty
                }
            }
            Ty::Value | Ty::Custom(_) => Ty::Value,
        }
    }

    fn one_of(&mut self, values: &[Value], hint: &str) -> Ty {
        if values.is_empty() {
            return Ty::Value;
        }
        if values.iter().all(Value::is_string) {
            let mut unique: Vec<String> = Vec::new();
            for value in values {
                let value = value.as_str().unwrap_or_default().to_owned();
                if !unique.contains(&value) {
                    unique.push(value);
                }
            }
            let name = self.unique_name(hint);
            self.defs.push(TypeDef::StrEnum {
                name: name.clone(),
                values: unique,
            });
            return Ty::Named(name);
        }
        if values.iter().all(Value::is_i64) {
            Ty::I64
        } else if values.iter().all(Value::is_number) {
            Ty::F64
        } else if values.iter().all(Value::is_boolean) {
            Ty::Bool
        } else {
            Ty::Value
        }
    }

    // An enum of the variants when every schema has the selector field and
    // keeps its value, an object otherwise
    fn tagged(&mut self, selector: &Value, schemas: &Value, hint: &str) -> Ty {
        let (Value::String(selector), Value::Object(schemas)) = (selector, schemas) else {
            return Ty::Map;
        };
        // An empty list of rules drops the field from the output, like a
        // field that is not in the schema
        let keeps_selector = |schema: &Value| {
            schema.get(selector).is_some_and(|rules| {
                let rules = rule_list(rules);
                !rules.is_empty()
                    && rules.iter().all(|rule| {
                        rule_parts(rule).is_some_and(|(name, _)| match self.resolve(name) {
                            Some(Resolved::Builtin(rule)) => SELECTOR_RULES.contains(&rule),
                            _ => false,
                        })
                    })
            })
        };
        if schemas.is_empty() || !schemas.values().all(keeps_selector) {
            return Ty::Map;
        }
        let name = self.unique_name(hint);
        let mut variants = Vec::new();
        for (value, schema) in schemas {
            let Value::Object(schema) = schema else {
                return Ty::Map;
            };
            let variant_name = self.unique_name(&format!("{name}{}", variant_name(value)));
            variants.push((
                value.clone(),
                self.object(schema, variant_name, Some(selector)),
            ));
        }
        self.defs.push(TypeDef::Tagged {
            name: name.clone(),
            selector: selector.clone(),
            variants,
        });
        Ty::Named(name)
    }

    fn or(&mut self, rule_sets: &[Value], state: State, hint: &str) -> State {
        let branches: Vec<State> = rule_sets
            .iter()
            .enumerate()
            .map(|(i, rules)| self.chain(rules, state.clone(), &format!("{hint}{}", i + 1)))
            .collect();
        let Some(first) = branches.first() else {
            return state;
        };
        let present = branches.iter().all(|branch| branch.present);
        if branches.iter().all(|branch| branch.ty == first.ty) {
            return State {
                ty: first.ty.clone(),
                present,
            };
        }
        let mut variants: Vec<Ty> = Vec::new();
        for branch in branches {
            if !variants.contains(&branch.ty) {
                variants.push(branch.ty);
            }
        }
        let name = self.unique_name(hint);
        self.defs.push(TypeDef::Union {
            name: name.clone(),
            variants,
        });
        State {
            ty: Ty::Named(name),
            present,
        }
    }
}
