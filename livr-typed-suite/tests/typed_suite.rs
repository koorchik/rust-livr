//! The LIVR test suite with typed schemas. The typed output has no empty
//! values: optional fields with `null` or `""` are `None`, so expected
//! outputs are compared without them.

use serde_json::Value;

include!(concat!(env!("OUT_DIR"), "/cases.rs"));

fn check_output<T: serde::Serialize + std::fmt::Debug>(
    result: Result<T, livr::Errors>,
    expected: &Value,
) {
    let output =
        result.unwrap_or_else(|errors| panic!("expected output {expected}, got errors {errors}"));
    let output = serde_json::to_value(&output).unwrap();
    assert!(
        json_eq(&without_empty(&output), &without_empty(expected)),
        "expected output {expected}, got {output}"
    );
}

fn check_errors<T: std::fmt::Debug>(result: Result<T, livr::Errors>, expected: &Value) {
    match result {
        Ok(output) => panic!("expected errors {expected}, got output {output:?}"),
        Err(errors) => {
            let errors = serde_json::to_value(&errors).unwrap();
            assert_eq!(&errors, expected);
        }
    }
}

fn without_empty(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(_, value)| !livr::util::is_no_value(value))
                .map(|(key, value)| (key.clone(), without_empty(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    if livr::util::is_no_value(item) {
                        Value::Null
                    } else {
                        without_empty(item)
                    }
                })
                .collect(),
        ),
        value => value.clone(),
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
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
