//! The LIVR test suites (tests/test_suite and tests/test_suite_2.5, copies
//! of the ones of the specification): every case validates `input.json` with
//! `rules.json` (and the aliases of `aliases.json`), and expects
//! `output.json` or `errors.json`. `options.json` fixes the current time
//! (`{"now": "2026-10-18T12:00:00Z"}`); the schemas of `schema_errors` (2.5)
//! must not build.
//!
//! Values are compared like in the Dart and Python implementations: numbers
//! by value (`1 == 1.0`), but a string is never equal to a number.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libtest_mimic::{Failed, Trial};
use livr_core::{Alias, Registry, Validator};
use serde_json::Value;

pub const SUITES: [&str; 4] = [
    "positive",
    "negative",
    "aliases_positive",
    "aliases_negative",
];

// The suite of the LIVR 2.5 draft: the same directories, and schema errors
const SUITES_2_5: [&str; 5] = [
    "positive",
    "negative",
    "aliases_positive",
    "aliases_negative",
    "schema_errors",
];

pub struct Case {
    pub name: String,
    pub dir: PathBuf,
    pub is_schema_error: bool,
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (root, prefix, suites) in [
        ("tests/test_suite", "", &SUITES[..]),
        ("tests/test_suite_2.5", "2.5/", &SUITES_2_5[..]),
    ] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(root);
        // The published crate has no copy of the suite (its license is not
        // the one of the crate): nothing to run
        if !root.is_dir() {
            eprintln!(
                "{} is missing: the LIVR test suite is skipped",
                root.display()
            );
            continue;
        }
        for &suite in suites {
            let mut dirs: Vec<_> = fs::read_dir(root.join(suite))
                .expect("test suite directory")
                .map(|entry| entry.expect("entry").path())
                .filter(|path| {
                    path.is_dir() && !path.file_name().unwrap().to_string_lossy().starts_with('.')
                })
                .collect();
            dirs.sort();
            for dir in dirs {
                let name = format!(
                    "{prefix}{suite}/{}",
                    dir.file_name().unwrap().to_string_lossy()
                );
                cases.push(Case {
                    name,
                    dir,
                    is_schema_error: suite == "schema_errors",
                });
            }
        }
    }
    cases
}

fn read(dir: &Path, file: &str) -> Option<Value> {
    let text = fs::read_to_string(dir.join(file)).ok()?;
    Some(serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}/{file}: {e}", dir.display())))
}

// `YYYY-MM-DDTHH:MM:SSZ`, the form of the `now` option of the suite
fn parse_now(now: &str) -> SystemTime {
    let number = |range: std::ops::Range<usize>| -> i64 { now[range].parse().expect("a number") };
    assert!(now.len() == 20 && now.ends_with('Z'), "now: {now}");
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    // Days since 1970-01-01, "days_from_civil" by Howard Hinnant
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds = days * 86_400 + number(11..13) * 3600 + number(14..16) * 60 + number(17..19);
    UNIX_EPOCH + Duration::from_secs(u64::try_from(seconds).expect("after 1970"))
}

/// Runs a case, with the rule names of the rules and aliases converted.
pub fn run_case(
    dir: &Path,
    is_schema_error: bool,
    convert: fn(&Value) -> Value,
) -> Result<(), String> {
    let rules = convert(&read(dir, "rules.json").expect("rules.json"));
    let mut registry = Registry::default();
    let built = register_aliases(dir, &mut registry, convert).and_then(|()| {
        let mut builder = Validator::builder(&rules).registry(&registry);
        if let Some(now) = read(dir, "options.json").and_then(|options| options.get("now").cloned())
        {
            builder = builder.now(parse_now(now.as_str().expect("now is a string")));
        }
        builder.build().map_err(|e| format!("exception: {e}"))
    });
    if is_schema_error {
        return match built {
            Ok(_) => Err("expected a schema error, the validator was built".to_owned()),
            Err(_) => Ok(()),
        };
    }
    let validator = built?;
    let input = read(dir, "input.json").expect("input.json");
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

fn register_aliases(
    dir: &Path,
    registry: &mut Registry,
    convert: fn(&Value) -> Value,
) -> Result<(), String> {
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
    Ok(())
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
            let Case {
                name,
                dir,
                is_schema_error,
            } = case;
            Trial::test(name, move || {
                run_case(&dir, is_schema_error, convert).map_err(Failed::from)
            })
        })
        .collect()
}
