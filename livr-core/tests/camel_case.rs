//! The LIVR test suites again, with the camelCase names of the rules.

mod common;

use livr_core::util::camelize;
use serde_json::{Map, Value};

// Converts the rule names of a schema, not the field names
fn camelize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, rules)| (name.clone(), camelize_rules(rules)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn camelize_rules(rules: &Value) -> Value {
    match rules {
        Value::Array(rules) => Value::Array(rules.iter().map(camelize_rule).collect()),
        rule => camelize_rule(rule),
    }
}

fn camelize_rule(rule: &Value) -> Value {
    match rule {
        Value::String(name) => Value::String(camelize(name)),
        Value::Object(rule) => {
            let mut camelized = Map::new();
            for (name, args) in rule {
                let args = match name.as_str() {
                    "nested_object" | "list_of_objects" => camelize_schema(args),
                    "list_of" | "values" | "keys" => match args {
                        // The old syntax: one list of rules
                        Value::Array(items) if items.len() == 1 && items[0].is_array() => {
                            Value::Array(vec![camelize_rules(&items[0])])
                        }
                        other => camelize_rules(other),
                    },
                    // The reference stays, the rules of the condition form
                    // or of the cases are converted
                    "when" => match args {
                        Value::Array(items) if items.len() == 2 => {
                            let cases = match &items[1] {
                                Value::Object(cases) => Value::Object(
                                    cases
                                        .iter()
                                        .map(|(key, rules)| (key.clone(), camelize_rules(rules)))
                                        .collect(),
                                ),
                                other => other.clone(),
                            };
                            Value::Array(vec![items[0].clone(), cases])
                        }
                        Value::Array(items) if !items.is_empty() => Value::Array(
                            std::iter::once(items[0].clone())
                                .chain(items[1..].iter().map(camelize_rules))
                                .collect(),
                        ),
                        other => other.clone(),
                    },
                    "or" => match args {
                        Value::Array(sets) => {
                            Value::Array(sets.iter().map(camelize_rules).collect())
                        }
                        other => other.clone(),
                    },
                    "variable_object" | "list_of_different_objects" => match args {
                        Value::Array(items) if items.len() == 2 => {
                            let schemas = match &items[1] {
                                Value::Object(schemas) => Value::Object(
                                    schemas
                                        .iter()
                                        .map(|(key, schema)| (key.clone(), camelize_schema(schema)))
                                        .collect(),
                                ),
                                other => other.clone(),
                            };
                            Value::Array(vec![items[0].clone(), schemas])
                        }
                        other => other.clone(),
                    },
                    _ => args.clone(),
                };
                camelized.insert(camelize(name), args);
            }
            Value::Object(camelized)
        }
        other => other.clone(),
    }
}

fn main() {
    let args = libtest_mimic::Arguments::from_args();
    libtest_mimic::run(&args, common::trials(camelize_schema)).exit();
}
