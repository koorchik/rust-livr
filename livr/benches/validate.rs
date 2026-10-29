//! The scenarios of the LIVR benchmarks (js-validator-livr, dart-livr): a
//! simple form and a nested object, with a validator built once (static) or
//! for every validation (dynamic), and a form where every field is invalid.

use std::hint::black_box;
use std::time::{Duration, Instant};

use livr::Validator;
use serde_json::{Value, json};

fn simple_schema() -> Value {
    json!({
        "name": ["required", "string"],
        "email": ["required", "email"],
        "age": ["required", "positive_integer"],
        "gender": {"one_of": ["male", "female"]},
        "password": ["required", {"min_length": 8}],
    })
}

fn simple_input(i: usize) -> Value {
    json!({
        "name": "John Doe",
        "email": format!("john.doe{i}@example.com"),
        "age": if i.is_multiple_of(2) { json!(25) } else { json!("25") },
        "gender": "male",
        "password": "secret-password",
        "extra": "stripped",
    })
}

fn invalid_simple_input(i: usize) -> Value {
    json!({
        "name": "",
        "email": format!("john.doe{i}@"),
        "age": -1,
        "gender": "unknown",
        "password": "short",
    })
}

fn nested_schema() -> Value {
    json!({
        "id": ["required", "positive_integer"],
        "user": ["required", {"nested_object": {
            "name": ["required", "string"],
            "address": ["required", {"nested_object": {
                "city": ["required", "string"],
                "zip": ["required", {"like": "^\\d{5}$"}],
            }}],
        }}],
        "tags": {"list_of": ["required", "string"]},
    })
}

fn nested_input(i: usize) -> Value {
    json!({
        "id": i + 1,
        "user": {"name": "John Doe", "address": {"city": "Kyiv", "zip": "01001", "extra": "stripped"}},
        "tags": ["rust", "livr", "fast"],
    })
}

// The simple form as a type
livr::schema! {
    struct SimpleForm {
        name: ["required", "string"],
        email: ["required", "email"],
        age: ["required", "positive_integer"],
        gender: {"one_of": ["male", "female"]},
        password: ["required", {"min_length": 8}],
    }
}

livr::schema! {
    struct NestedForm {
        id: ["required", "positive_integer"],
        user: ["required", {"nested_object": {
            name: ["required", "string"],
            address: ["required", {"nested_object": {
                city: ["required", "string"],
                zip: ["required", {"like": "^\\d{5}$"}],
            }}],
        }}],
        tags: {"list_of": ["required", "string"]},
    }
}

const POOL: usize = 1000;

type Case<'a> = (&'static str, Box<dyn FnMut(usize) -> bool + 'a>);

// Best of 5 runs of 0.5 s after 0.5 s of warm-up, like bench-js and the
// Dart benchmark: the best run is the one least disturbed by other processes
fn measure(mut run: impl FnMut(usize) -> bool) -> f64 {
    let mut timed = |duration: Duration| {
        let start = Instant::now();
        let mut ops = 0;
        while start.elapsed() < duration {
            for _ in 0..1000 {
                black_box(run(ops % POOL));
                ops += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let ops = ops as f64;
        ops / start.elapsed().as_secs_f64()
    };
    timed(Duration::from_millis(500));
    (0..5)
        .map(|_| timed(Duration::from_millis(500)))
        .fold(0.0, f64::max)
}

fn main() {
    let filter = std::env::args().skip(1).find(|arg| !arg.starts_with('-'));
    let simple = simple_schema();
    let nested = nested_schema();
    let simple_validator = Validator::new(&simple).unwrap();
    let nested_validator = Validator::new(&nested).unwrap();
    let simple_inputs: Vec<Value> = (0..POOL).map(simple_input).collect();
    let nested_inputs: Vec<Value> = (0..POOL).map(nested_input).collect();
    let invalid_inputs: Vec<Value> = (0..POOL).map(invalid_simple_input).collect();
    let simple_json: Vec<String> = simple_inputs.iter().map(Value::to_string).collect();
    let nested_json: Vec<String> = nested_inputs.iter().map(Value::to_string).collect();
    assert!(
        simple_inputs
            .iter()
            .all(|input| simple_validator.validate(input).is_ok())
    );
    assert!(
        nested_inputs
            .iter()
            .all(|input| nested_validator.validate(input).is_ok())
    );
    assert!(
        invalid_inputs
            .iter()
            .all(|input| simple_validator.validate(input).is_err())
    );
    assert!(
        simple_inputs
            .iter()
            .all(|input| SimpleForm::validate(input).is_ok())
    );
    assert!(
        nested_inputs
            .iter()
            .all(|input| NestedForm::validate(input).is_ok())
    );

    let cases: Vec<Case<'_>> = vec![
        (
            "simple, static",
            Box::new(|i| simple_validator.validate(&simple_inputs[i]).is_ok()),
        ),
        (
            "simple, dynamic",
            Box::new(|i| {
                Validator::new(&simple)
                    .unwrap()
                    .validate(&simple_inputs[i])
                    .is_ok()
            }),
        ),
        (
            "nested, static",
            Box::new(|i| nested_validator.validate(&nested_inputs[i]).is_ok()),
        ),
        (
            "nested, dynamic",
            Box::new(|i| {
                Validator::new(&nested)
                    .unwrap()
                    .validate(&nested_inputs[i])
                    .is_ok()
            }),
        ),
        (
            "invalid, static",
            Box::new(|i| simple_validator.validate(&invalid_inputs[i]).is_err()),
        ),
        (
            "simple, typed",
            Box::new(|i| SimpleForm::validate(&simple_inputs[i]).is_ok()),
        ),
        (
            "nested, typed",
            Box::new(|i| NestedForm::validate(&nested_inputs[i]).is_ok()),
        ),
        // Data from a request body: parsed, then validated
        (
            "simple, static, JSON string",
            Box::new(|i| {
                let data: Value = serde_json::from_str(&simple_json[i]).unwrap();
                simple_validator.validate_owned(data).is_ok()
            }),
        ),
        (
            "nested, static, JSON string",
            Box::new(|i| {
                let data: Value = serde_json::from_str(&nested_json[i]).unwrap();
                nested_validator.validate_owned(data).is_ok()
            }),
        ),
        (
            "simple, typed, JSON string",
            Box::new(|i| {
                let data: Value = serde_json::from_str(&simple_json[i]).unwrap();
                SimpleForm::validate_owned(data).is_ok()
            }),
        ),
        (
            "nested, typed, JSON string",
            Box::new(|i| {
                let data: Value = serde_json::from_str(&nested_json[i]).unwrap();
                NestedForm::validate_owned(data).is_ok()
            }),
        ),
    ];
    println!("| Scenario | livr (Rust) |\n|---|---|");
    for (name, run) in cases {
        if filter
            .as_deref()
            .is_some_and(|filter| !name.contains(filter))
        {
            continue;
        }
        let ops = measure(run);
        println!("| {name} | {:.2} M/s |", ops / 1e6);
    }
}
