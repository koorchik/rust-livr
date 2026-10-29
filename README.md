# livr

[![CI](https://github.com/koorchik/rust-livr/actions/workflows/test.yml/badge.svg)](https://github.com/koorchik/rust-livr/actions/workflows/test.yml)
[![crates.io](https://img.shields.io/crates/v/livr.svg)](https://crates.io/crates/livr)
[![docs.rs](https://img.shields.io/docsrs/livr)](https://docs.rs/livr)
[![LIVR 2.0](https://img.shields.io/badge/LIVR-2.0-blue)](https://livr-spec.org)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/koorchik/rust-livr/blob/master/LICENSE)

A Rust implementation of the [LIVR 2.0 specification](https://livr-spec.org) (Language Independent Validation Rules). It passes the full official test suite: all 70 cases, also with camelCase rule names and as typed schemas.

**Every error of every field, in one response.** livr never stops at the first error: it checks all the fields, in nested objects and in every item of a list, and gives an error code for each invalid one, the type errors too. The frontend highlights every wrong field at once.

One JSON schema for validation, clean data and Rust types. The same schema works in [12 other languages](https://livr-spec.org/introduction/implementations.html), with the same error codes.

```rust
livr::schema! {
    #[derive(Debug)]
    pub struct SignUp {
        name: ["trim", "required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
    }
}

fn main() -> Result<(), livr::Errors> {
    // As a form sends it: spaces, upper case, a number as a string
    let user = SignUp::validate(&serde_json::json!({"name": "  Jane ", "email": "JANE@EXAMPLE.COM", "age": "42"}))?;
    assert_eq!((user.name.as_str(), user.email.as_str(), user.age), ("Jane", "jane@example.com", 42));

    // Every field gets its error code, the type errors too
    let errors = SignUp::validate(&serde_json::json!({"email": "wrong", "age": "abc"})).unwrap_err();
    assert_eq!(errors.to_string(), r#"{"age":"NOT_POSITIVE_INTEGER","email":"WRONG_EMAIL","name":"REQUIRED"}"#);
    Ok(())
}
```

## Why livr

- **All errors in one pass.** Validation never stops at the first invalid field: every field is checked, in nested objects and lists too, type errors included. A form can highlight all its invalid fields at once.
- **Error codes, not messages.** `{"address": {"zip": "WRONG_FORMAT"}}`: shaped like the data, easy to translate and to map to the UI.
- **Strict by default.** Fields without rules are removed from the output: data that is not explicitly allowed never reaches your code.
- **Cleans as it validates.** `trim`, `to_lc`, `default`; `"25"` from a form or a query string becomes `25`.
- **Declarative and portable.** Rules are plain JSON: store them, load them at run time, share one schema with services in other languages, with the same output and error codes.
- **Extensible.** Your own rules, and aliases with their own error codes.
- **Typed in Rust.** `schema!` checks the schema at compile time and infers structs and enums from the rules; [livr-axum](https://github.com/koorchik/rust-livr-axum) brings it to axum handlers.

## All the errors at once

Each field gets the code of its first failing rule. All the fields are checked, at every level:

```rust
livr::schema! {
    pub struct Order {
        email: ["required", "email"],
        address: ["required", {"nested_object": {
            city: ["required", "string"],
            zip: ["required", {"like": "^\\d{5}$"}],
        }}],
        items: ["required", {"list_of_objects": {
            sku: ["required", "string"],
            quantity: ["required", "positive_integer"],
        }}],
    }
}

let errors = Order::validate(&serde_json::json!({
    "email": "wrong",
    "address": {"zip": "1"},
    "items": [{"sku": "A-1", "quantity": 2}, {"quantity": "many"}],
}))
.err()
.unwrap();

// Shaped like the data: an object for an object, a list for a list (null for a valid item)
assert_eq!(serde_json::to_value(errors).unwrap(), serde_json::json!({
    "email": "WRONG_EMAIL",
    "address": {"city": "REQUIRED", "zip": "WRONG_FORMAT"},
    "items": [null, {"sku": "REQUIRED", "quantity": "NOT_POSITIVE_INTEGER"}],
}));
```

A frontend puts each code next to its field: `errors.address.zip`, `errors.items[1].quantity`. The codes are machine codes (`REQUIRED`, `TOO_LONG`, `WRONG_EMAIL`), the same in every LIVR implementation: the frontend translates them.

## Compared with other Rust validators

| | **livr** | [validator](https://crates.io/crates/validator) | [garde](https://crates.io/crates/garde) | [jsonschema](https://crates.io/crates/jsonschema) |
|---|---|---|---|---|
| All errors at once, type errors too | ✅ | ❌ serde stops at the first wrong type | ❌ serde stops at the first wrong type | ✅ |
| Error codes shaped like the data | ✅ `{"address": {"zip": "WRONG_FORMAT"}}` | ⚠️ lists of rule names and parameters: `"length"` for an empty name | ❌ text: `address.zip: length is lower than 5` | ❌ text with JSON pointers |
| Converts and cleans the input: `"42"` → `42`, trim, lower case | ✅ | ❌ the input must fit the types | ❌ the input must fit the types | ❌ validates only |
| Rules are data (JSON) | ✅ | ❌ Rust attributes | ❌ Rust attributes | ✅ |
| Schemas loaded at run time | ✅ | ❌ | ❌ | ✅ |
| Same rules and error codes in other languages | ✅ 13 implementations, one test suite | ❌ | ❌ | ⚠️ same rules, messages differ between implementations |
| Rust types | ✅ inferred from the rules | ✅ the struct you write | ✅ the struct you write | ❌ `serde_json::Value` |
| Schema checked at compile time | ✅ | ✅ | ✅ | ❌ at run time |
| axum | ✅ [livr-axum](https://github.com/koorchik/rust-livr-axum) | axum-valid | axum-valid | |

What a client gets for the same sign-up form (validator 0.21, garde 0.23 and jsonschema 0.60 with format checks on; the form of each is in its own syntax):

```text
{"name": "", "email": "wrong", "age": "abc", "address": {"zip": "1"}}

livr        {"address":{"zip":"WRONG_FORMAT"},"age":"NOT_POSITIVE_INTEGER","email":"WRONG_EMAIL","name":"REQUIRED"}
validator   invalid type: string "abc", expected i64      (serde, before validation: one error)
garde       invalid type: string "abc", expected i64      (serde, before validation: one error)
jsonschema  /address/zip "1" does not match "^[0-9]{5}$"
            /age "abc" is not of type "integer"
            /email "wrong" is not a "email"
            /name "" is shorter than 1 character

{"name": "  Jane ", "email": "JANE@EXAMPLE.COM", "age": "42", "address": {"zip": "01001"}}

livr        SignUp { name: "Jane", email: "jane@example.com", age: 42, address: SignUpAddress { zip: "01001" } }
validator   invalid type: string "42", expected i64
garde       invalid type: string "42", expected i64
jsonschema  /age "42" is not of type "integer"
```

When another crate fits better:
- **jsonschema** when the contract must be JSON Schema (OpenAPI), or the data is recursive (`$ref`): LIVR has no recursion.
- **validator** or **garde** when the data is already a typed struct (a config file, internal messages) and only its values need checks: they have large ecosystems. livr is a new crate.

## Install

```toml
[dependencies]
livr = "0.1"
serde_json = "1"
```

## Typed schemas: `schema!`

The schema is written as JSON in the macro. Keys of nested objects can be identifiers. The macro checks it with the same engine as the run time, so a mistyped rule or a bad argument is a compile error on its field. It then generates a struct whose types follow from the rules:

```rust
livr::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct SignUp {
        name: ["required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
        gender: {"one_of": ["male", "female"]},
        address: ["required", {"nested_object": {
            city: ["required", "string"],
            zip: ["required", {"like": "^\\d{5}$"}],
        }}],
        tags: {"list_of": ["required", "string"]},
    }
}

// pub struct SignUp {
//     pub name: String,
//     pub email: String,
//     pub age: i64,
//     pub gender: Option<SignUpGender>,   // enum SignUpGender { Male, Female }
//     pub address: SignUpAddress,         // struct SignUpAddress { city: String, zip: String }
//     pub tags: Option<Vec<String>>,
// }

fn main() -> Result<(), livr::Errors> {
    let user = SignUp::validate(&serde_json::json!({
        "name": "Jane", "email": "JANE@EXAMPLE.COM", "age": "42",
        "address": {"city": "Kyiv", "zip": "01001"},
    }))?;
    assert_eq!(user.email, "jane@example.com");
    assert_eq!(user.age, 42);
    Ok(())
}
```

- `validate(&Value)` borrows the data. `validate_owned(Value)` takes data you own, such as a parsed request body, and moves values instead of copying them.
- Errors are the LIVR error codes, shaped like the data: `{"email": "WRONG_EMAIL", "address": {"zip": "REQUIRED"}}`.
- The generated types implement `serde::Serialize` and serialize back to the output of the schema. `SignUp::SCHEMA` is the schema as JSON, and `SignUp::validator()` validates without types.
- The main type implements `livr::Schema`, for code generic over schemas, like the extractors of livr-axum.
- The schema can come from a JSON file, for example one shared with other services: `livr::schema!(pub struct Order = "schemas/order.json");`.
- Generated types are named after the path to them: `SignUpAddress` for the field `address` of `SignUp`, with a number when a name is taken. `field: rules as Name` names the type of a field yourself, for example when a generated name meets another type of the module.
- `SignUp::FIELDS` lists the fields and tells which are lists, for code that prepares a form or a query string before validation.

### Types of the output

Each field's rules are folded into a type and a presence, in order:

| Rules | Type |
|---|---|
| `string`, `email`, `url`, `iso_date`, `like`, length rules | `String` (an enum from an earlier `one_of` stays) |
| `integer`, `positive_integer` | `i64` |
| `decimal`, `positive_decimal` | `f64` |
| `min_number`, `max_number`, `number_between` | the earlier number type, else `f64` |
| `one_of` of strings | a generated enum (`as_str()`, `Display`) |
| `one_of` / `eq` of other values | `i64`, `f64`, `bool`, or `Value` when mixed |
| `nested_object` | a generated struct |
| `list_of`, `list_of_objects` | `Vec<item type>` |
| `variable_object`, `list_of_different_objects` | an enum tagged by the selector field, when every schema has the selector field and keeps its value; `Map` otherwise |
| `or` | the type of the rule sets when it is the same, else an untagged enum |
| `any_object` | `Map<String, Value>` |
| `trim`, `to_lc`, `to_uc`, `remove`, `leave_only` | `String`; `Value` when the earlier type is unknown, since objects pass through them unchanged |
| `default` | the earlier type |
| no rule that tells the type | `Value` |

A field is `T` when a rule makes it present (`required`, `not_empty_list`, a non-empty `default`), and `Option<T>` otherwise. An empty value (missing, `null` or `""`) of an optional field is `None`. `trim`, `remove` and `leave_only` can leave an empty string, so after them a field is optional again until another `required`.

The types are sound: for every valid data, the output fits them. A custom rule that outputs another type than it declares cannot panic the conversion; it gives `FORMAT_ERROR` for that field.

### Custom rules and aliases in `schema!`

```rust
use livr::{CompileCtx, Outcome, Rule, SchemaError, Value, rule_fn, util::string_arg};

fn starts_with(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<impl Rule + use<>, SchemaError> {
    let prefix = string_arg(args, 0, "starts_with")?;
    Ok(rule_fn(move |value, _ctx| match value.as_str() {
        Some(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
        _ => Outcome::Valid,
    }))
}

livr::schema! {
    #[rule(starts_with = starts_with, output = String)]
    #[alias(adult_age = ["positive_integer", {"min_number": 18}], error = "WRONG_AGE")]
    pub struct Person {
        code: ["required", {"starts_with": "X-"}],   // String
        age: ["required", "adult_age"],              // i64
    }
}
```

`#[rule(...)]` takes `output = Type` (the type does not change when it is left out) and `present` (the rule makes the field present). The output type implements `livr::FromLivr` and `livr::FromLivrRef`.

## Schemas known at run time: `Validator`

```rust
use livr::Validator;
use serde_json::json;

fn main() -> Result<(), livr::SchemaError> {
    let validator = Validator::new(&json!({
        "name": "required",
        "email": ["required", "email"],
        "age": "positive_integer",
    }))?;

    assert_eq!(
        validator.validate(&json!({"name": "Jane", "email": "jane@example.com", "age": "42", "extra": 1})),
        Ok(json!({"name": "Jane", "email": "jane@example.com", "age": 42})),
    );
    let errors = validator.validate(&json!({"email": "wrong"})).unwrap_err();
    assert_eq!(errors.to_string(), r#"{"email":"WRONG_EMAIL","name":"REQUIRED"}"#);
    Ok(())
}
```

- `Validator::new` compiles the schema once. An invalid schema (unknown rule, bad arguments, recursive alias) is a `SchemaError` here, never during validation.
- A `Validator` is immutable, `Send + Sync`, and cheap to clone. It has no state per validation.
- `validate_owned(Value)` moves the values of owned data to the output. `validate_into::<T: Deserialize>` deserializes the output.
- `Validator::builder(&schema).registry(&registry).auto_trim(true).build()` sets custom rules, aliases and auto-trim (trimming all strings of the data first; off by default).

### Custom rules and aliases

There is no global registry. A `Registry` value has the built-in rules, plus the custom rules and aliases you add:

```rust
use livr::{Alias, Outcome, Registry, Validator, rule_builder, rule_fn};
use serde_json::json;

fn main() -> Result<(), livr::SchemaError> {
    let mut registry = Registry::default();
    registry.register_rule("not_admin", rule_builder(|_args, _ctx| {
        Ok(rule_fn(|value, _ctx| {
            if value == "admin" { Outcome::code("NOT_ALLOWED") } else { Outcome::Valid }
        }))
    }));
    registry.register_alias(
        Alias::new("adult_age", json!(["positive_integer", {"min_number": 18}])).with_error("WRONG_AGE"),
    )?;

    let validator = Validator::builder(&json!({"login": "not_admin", "age": "adult_age"}))
        .registry(&registry)
        .build()?;
    assert!(validator.validate(&json!({"login": "admin", "age": 12})).is_err());
    Ok(())
}
```

A rule builder gets the arguments of the rule in the schema and returns the rule. Expensive work, such as parsing arguments and compiling nested rules with `CompileCtx::compile_field` and `compile_object`, happens once in the builder. A rule returns `Outcome::Valid`, `Outcome::Value(new value)` or `Outcome::Error(...)`. `ctx.field(name)` gives the raw value of another field of the same object. Every rule is also registered under its camelCase name (`minLength`).

## Semantics

These follow the LIVR specification and its test suite. Where the specification leaves something open, this implementation is strict, like dart-livr and the LIVR 3 draft:

- **Numbers in strings:** they must be decimal, `-?[0-9]+(\.[0-9]+)?`. `" 5"`, `"+5"`, `"1e3"`, `"0x10"` and `"Infinity"` are not numbers.
- **Booleans** are not numbers.
- **Integers** fit `i64`. Integer rules output `i64`, decimal rules output `f64`.
- **Lengths** are counted in characters (Unicode code points), not UTF-16 units.
- **Rule objects** have exactly one rule name. `{"required": [], "email": []}` is a schema error.
- **`like`** takes JavaScript regular expressions (without the `u` flag) and matches like JavaScript: `\d`, `\w` and `\b` are ASCII only, also with the `i` flag (`k` matches `K` but not the Kelvin sign `K`); `\s` is the whitespace of JavaScript; `.` does not match line terminators; an escaped letter without a meaning is the letter (`\z` is `z`, `\p{L}` is `p{L}`). Look-around, backreferences and inline flags are schema errors. A character beyond U+FFFF is one character, not two UTF-16 units as in JavaScript. Matching takes linear time.
- **Numbers to strings** follow JavaScript: `10.0` is `"10"`.

## axum: `livr-axum`

[livr-axum](https://github.com/koorchik/rust-livr-axum) has extractors for [axum](https://github.com/tokio-rs/axum): `Livr<T>` (a JSON body), `LivrQuery<T>`, `LivrForm<T>`, a middleware for schemas known at run time, and a route that serves a schema to the frontend:

```rust,ignore
async fn sign_up(Livr(user): Livr<SignUp>) -> String {
    format!("Hello, {}", user.name)
}
```

An invalid request gets `422` with all the errors of the fields: `{"code": "VALIDATION_ERROR", "fields": {"email": "WRONG_EMAIL"}}`. Examples are in [its repository](https://github.com/koorchik/rust-livr-axum#examples).

## Performance

These are the scenarios of the LIVR benchmarks, on one core of an i9-13900H. Full numbers and the method are in [BENCHMARKS.md](https://github.com/koorchik/rust-livr/blob/master/BENCHMARKS.md).

| Scenario | Validations per second |
|---|---|
| Simple form (5 fields), typed (`schema!`) | 9.3 M/s |
| Nested object (2 levels and a list), typed (`schema!`) | 4.6 M/s |
| Simple form, from a JSON string, typed | 2.97 M/s |
| Nested object, from a JSON string, `Validator` | 1.50 M/s |
| Simple form, `Validator`, borrowed data | 4.01 M/s |
| First validation in a new process (cold start), simple form | 7 µs |

These are built with `lto = "fat"` and `codegen-units = 1`, as in the release profile of this repository. With the default release profile (no fat LTO), the same scenarios were 9–20% slower: set both in the release profile of your application for the numbers above.

The typed path is the fast one. Its rules are compiled ahead of time: the generated code calls each rule directly and validates straight into the struct, with no output map. `Validator::validate` on borrowed data copies every key and value of the output into a new `serde_json::Value`; `validate_owned` and the types of `schema!` move them instead.

## Crates

- `livr`: what you use. It re-exports `livr-core` and `livr-macros`.
- `livr-core`: the engine (`Validator`, rules, `Registry`).
- `livr-macros`: `schema!`.
- `livr-wasm` (not published): a WebAssembly build.

## Tests

`cargo test --workspace` runs the LIVR test suite (`livr-core/tests/test_suite`, a copy of the one in the specification, under its own license, Artistic-2.0; it is not in the published crates):
- with borrowed and owned data;
- again with camelCase rule names;
- with every case as a `schema!` (`livr-typed-suite`);
- plus compile-fail tests of `schema!` (`livr/tests/ui`).

To refresh the suite: `cp -r ../LIVR/test_suite livr-core/tests/`.

## License

[MIT](https://github.com/koorchik/rust-livr/blob/master/LICENSE). The copy of the LIVR test suite in `livr-core/tests/test_suite` is under the [Artistic License 2.0](https://livr-spec.org/license-and-copyright.html), like the specification.
