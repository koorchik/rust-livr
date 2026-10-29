//! The LIVR test suite (tests/test_suite, a copy of the one of the
//! specification): every case validates `input.json` with `rules.json` (and
//! the aliases of `aliases.json`), and expects `output.json` or
//! `errors.json`.
//!
//! Values are compared like in the Dart and Python implementations: numbers
//! by value (`1 == 1.0`), but a string is never equal to a number.

use std::fs;
use std::path::{Path, PathBuf};

use libtest_mimic::{Failed, Trial};
use livr_core::{Alias, Registry, Validator};
use serde_json::Value;

pub const SUITES: [&str; 4] = [
    "positive",
    "negative",
    "aliases_positive",
    "aliases_negative",
];

pub struct Case {
    pub name: String,
    pub dir: PathBuf,
}

pub fn cases() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_suite");
    // The published crate has no copy of the suite (its license is not the
    // one of the crate): nothing to run
    if !root.is_dir() {
        eprintln!(
            "{} is missing: the LIVR test suite is skipped",
            root.display()
        );
        return Vec::new();
    }
    let mut cases = Vec::new();
    for suite in SUITES {
        let mut dirs: Vec<_> = fs::read_dir(root.join(suite))
            .expect("test suite directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| {
                path.is_dir() && !path.file_name().unwrap().to_string_lossy().starts_with('.')
            })
            .collect();
        dirs.sort();
        for dir in dirs {
            let name = format!("{suite}/{}", dir.file_name().unwrap().to_string_lossy());
            cases.push(Case { name, dir });
        }
    }
    cases
}

fn read(dir: &Path, file: &str) -> Option<Value> {
    let text = fs::read_to_string(dir.join(file)).ok()?;
    Some(serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}/{file}: {e}", dir.display())))
}

/// Runs a case, with the rule names of the rules and aliases converted.
pub fn run_case(dir: &Path, convert: fn(&Value) -> Value) -> Result<(), String> {
    let rules = convert(&read(dir, "rules.json").expect("rules.json"));
    let input = read(dir, "input.json").expect("input.json");
    let mut registry = Registry::default();
    if let Some(Value::Array(aliases)) = read(dir, "aliases.json") {
        for alias in aliases {
            let alias = Alias::from_json(&alias).map_err(|e| e.to_string())?;
            let mut converted = Alias::new(alias.name(), convert_rules(alias.rules(), convert));
            if let Some(error) = alias.error() {
                converted = converted.with_error(error);
            }
            registry
                .register_alias(converted)
                .map_err(|e| e.to_string())?;
        }
    }
    let validator = Validator::builder(&rules)
        .registry(&registry)
        .build()
        .map_err(|e| format!("exception: {e}"))?;
    let result = validator.validate(&input);
    // Owned data must give the same result
    let owned_result = validator.validate_owned(input.clone());
    if result != owned_result {
        return Err(format!(
            "validate_owned gave {owned_result:?}, validate gave {result:?}"
        ));
    }
    if let Some(expected) = read(dir, "output.json") {
        match result {
            Ok(output) if json_eq(&output, &expected) => Ok(()),
            Ok(output) => Err(format!("expected output {expected}, got {output}")),
            Err(errors) => Err(format!("expected no errors, got {errors}")),
        }
    } else {
        let expected = read(dir, "errors.json").expect("errors.json");
        match result {
            Ok(output) => Err(format!("expected validation to fail, got output {output}")),
            Err(errors) => {
                let errors = serde_json::to_value(&errors).unwrap();
                if json_eq(&errors, &expected) {
                    Ok(())
                } else {
                    Err(format!("expected errors {expected}, got {errors}"))
                }
            }
        }
    }
}

fn convert_rules(rules: &Value, convert: fn(&Value) -> Value) -> Value {
    // The rules of a field: wrap them in a schema to convert them
    let schema = serde_json::json!({ "field": rules });
    convert(&schema)["field"].clone()
}

/// Equality of JSON values, with numbers compared by value.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| json_eq(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| json_eq(a, b)))
        }
        (a, b) => a == b,
    }
}

pub fn trials(convert: fn(&Value) -> Value) -> Vec<Trial> {
    cases()
        .into_iter()
        .map(|case| {
            let dir = case.dir;
            Trial::test(case.name, move || {
                run_case(&dir, convert).map_err(Failed::from)
            })
        })
        .collect()
}
