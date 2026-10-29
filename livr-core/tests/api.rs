//! The API beyond the test suite: custom rules, aliases, options, schema
//! errors, and semantics the suite does not cover.

use livr_core::{
    Alias, ErrorTree, Errors, Outcome, Registry, SchemaError, Validator, rule_builder, rule_fn,
};
use serde_json::{Map, Value, json};

fn validate(schema: Value, data: Value) -> Result<Value, Value> {
    Validator::new(&schema)
        .unwrap()
        .validate(&data)
        .map_err(|errors| serde_json::to_value(errors).unwrap())
}

#[test]
fn validator_is_send_sync_and_cheap_to_clone() {
    fn assert_send_sync<T: Send + Sync + Clone>() {}
    assert_send_sync::<Validator>();
    assert_send_sync::<Registry>();
    assert_send_sync::<Errors>();
}

#[test]
fn custom_rules_get_arguments_and_siblings() {
    let mut registry = Registry::default();
    registry.register_rule(
        "starts_with",
        rule_builder(|args, _ctx| {
            let prefix = livr_core::util::string_arg(args, 0, "starts_with")?;
            Ok(rule_fn(move |value, _ctx| match value {
                Value::String(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
                _ => Outcome::Valid,
            }))
        }),
    );
    registry.register_rule(
        "not_same_as_name",
        rule_builder(|_args, _ctx| {
            Ok(rule_fn(|value, ctx| {
                if ctx.field("name") == Some(value) {
                    Outcome::code("SAME_AS_NAME")
                } else {
                    Outcome::Valid
                }
            }))
        }),
    );
    let schema = json!({
        "code": {"starts_with": "X-"},
        "nick": "not_same_as_name",
        "items": {"list_of": {"starts_with": "I-"}},
    });
    let validator = Validator::builder(&schema)
        .registry(&registry)
        .build()
        .unwrap();
    assert!(
        validator
            .validate(&json!({"code": "X-1", "nick": "a", "name": "b", "items": ["I-1"]}))
            .is_ok()
    );
    let errors = validator
        .validate(&json!({"code": "Y-1", "nick": "a", "name": "a", "items": ["I-1", "J"]}))
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&errors).unwrap(),
        json!({"code": "WRONG_PREFIX", "nick": "SAME_AS_NAME", "items": [null, "WRONG_PREFIX"]})
    );
    // camelCase names are registered too
    assert!(registry.contains("startsWith"));
    // The built-in rules are not changed
    assert!(!Registry::builtin().contains("starts_with"));
}

#[test]
fn custom_rules_replace_built_in_ones() {
    let mut registry = Registry::default();
    registry.register_rule(
        "email",
        rule_builder(|_args, _ctx| Ok(rule_fn(|_, _| Outcome::code("NO_EMAILS")))),
    );
    let validator = Validator::builder(&json!({"a": "email", "b": "isoDate"}))
        .registry(&registry)
        .build()
        .unwrap();
    assert_eq!(
        validator
            .validate(&json!({"a": "x@y.org"}))
            .unwrap_err()
            .field("a"),
        Some(&ErrorTree::code("NO_EMAILS"))
    );
}

#[test]
fn aliases_can_use_later_aliases_and_report_recursion() {
    let mut registry = Registry::default();
    registry
        .register_alias(
            Alias::new("adult", json!(["positive_integer", "old_enough"])).with_error("WRONG_AGE"),
        )
        .unwrap();
    registry
        .register_alias(Alias::new("old_enough", json!({"min_number": 18})))
        .unwrap();
    let validator = Validator::builder(&json!({"age": "adult"}))
        .registry(&registry)
        .build()
        .unwrap();
    assert_eq!(
        validator.validate(&json!({"age": "20"})),
        Ok(json!({"age": 20}))
    );
    assert_eq!(
        validator
            .validate(&json!({"age": 12}))
            .unwrap_err()
            .field("age"),
        Some(&ErrorTree::code("WRONG_AGE"))
    );

    registry
        .register_alias(Alias::new("a", json!(["required", "b"])))
        .unwrap();
    registry
        .register_alias(Alias::new("b", json!({"list_of": "a"})))
        .unwrap();
    let error = Validator::builder(&json!({"x": "a"}))
        .registry(&registry)
        .build()
        .unwrap_err();
    assert_eq!(error.message(), "alias [a] is recursive");
}

#[test]
fn schema_errors_are_reported_when_building() {
    let message = |schema: Value| {
        Validator::new(&schema)
            .map(|_| ())
            .unwrap_err()
            .message()
            .to_owned()
    };
    assert_eq!(
        message(json!({"a": "unknown"})),
        "rule [unknown] not registered"
    );
    assert_eq!(
        message(json!({"a": {"max_length": "x"}})),
        "rule [max_length] expects a number, got \"x\""
    );
    assert_eq!(
        message(json!({"a": {"like": "(?=x)"}})).split(':').next(),
        Some("rule [like] got an invalid regular expression [(?=x)]")
    );
    assert_eq!(
        message(json!({"a": {"required": [], "string": []}})),
        "a rule object must have exactly one rule name, got {\"required\":[],\"string\":[]}"
    );
    assert_eq!(
        message(json!({"a": {"nested_object": [1]}})),
        "a schema must be an object, got 1"
    );
    assert_eq!(
        message(json!({"a": {"one_of": [[{}]]}})),
        "rule [one_of] expects primitive values, got {}"
    );
    assert!(matches!(
        Validator::new(&json!([])),
        Err(SchemaError { .. })
    ));
}

#[test]
fn data_that_is_not_an_object_is_a_format_error() {
    let validator = Validator::new(&json!({"a": "required"})).unwrap();
    for data in [json!(null), json!([]), json!("x"), json!(1)] {
        assert_eq!(validator.validate(&data), Err(Errors::Format));
    }
    assert_eq!(Errors::Format.to_string(), "\"FORMAT_ERROR\"");
}

#[test]
fn auto_trim_trims_all_strings() {
    let schema =
        json!({"name": {"max_length": 3}, "tags": {"list_of": "string"}, "obj": "any_object"});
    let validator = Validator::builder(&schema).auto_trim(true).build().unwrap();
    assert_eq!(
        validator.validate(&json!({"name": " abc ", "tags": [" a"], "obj": {"x": " y "}})),
        Ok(json!({"name": "abc", "tags": ["a"], "obj": {"x": "y"}}))
    );
    assert!(
        Validator::new(&schema)
            .unwrap()
            .validate(&json!({"name": " abc "}))
            .is_err()
    );
}

#[test]
fn numbers_are_strict() {
    let schema = json!({"n": "integer"});
    for value in [
        json!(" 5"),
        json!("5 "),
        json!("0x10"),
        json!("1e3"),
        json!("+5"),
        json!(true),
        json!("Infinity"),
    ] {
        assert_eq!(
            validate(schema.clone(), json!({"n": value})),
            Err(json!({"n": "NOT_INTEGER"})),
            "{value}"
        );
    }
    assert_eq!(
        validate(schema.clone(), json!({"n": "10.0"})),
        Ok(json!({"n": 10}))
    );
    assert_eq!(validate(schema, json!({"n": 10.0})), Ok(json!({"n": 10})));
    assert_eq!(
        validate(json!({"n": "decimal"}), json!({"n": "-1.5"})),
        Ok(json!({"n": -1.5}))
    );
    assert_eq!(
        validate(json!({"n": {"max_number": 10}}), json!({"n": true})),
        Err(json!({"n": "NOT_NUMBER"}))
    );
    assert_eq!(
        validate(json!({"n": {"min_number": 1}}), json!({"n": "5"})),
        Ok(json!({"n": 5}))
    );
}

#[test]
fn lengths_are_in_characters() {
    let schema = json!({"s": {"length_equal": 2}});
    assert!(validate(schema.clone(), json!({"s": "😀😀"})).is_ok());
    assert!(validate(json!({"s": {"max_length": 2}}), json!({"s": "Ще"})).is_ok());
    assert_eq!(
        validate(json!({"s": {"min_length": 3}}), json!({"s": "ЩеЩ"})),
        Ok(json!({"s": "ЩеЩ"}))
    );
    assert_eq!(
        validate(json!({"s": {"min_length": 3}}), json!({"s": "Ще"})),
        Err(json!({"s": "TOO_SHORT"}))
    );
}

#[test]
fn null_fields_stay_in_the_output() {
    assert_eq!(
        validate(json!({"a": "string", "b": "string"}), json!({"a": null})),
        Ok(json!({"a": null}))
    );
    assert_eq!(
        validate(json!({"a": {"default": 5}}), json!({})),
        Ok(json!({"a": 5}))
    );
}

#[test]
fn like_uses_ascii_classes_like_javascript() {
    let schema = json!({"s": {"like": "^\\w+\\b"}});
    assert!(validate(schema.clone(), json!({"s": "abc_1"})).is_ok());
    assert!(validate(schema, json!({"s": "щ"})).is_err());
}

#[test]
fn validate_into_deserializes_the_output() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct User {
        name: String,
        age: u32,
    }
    let validator =
        Validator::new(&json!({"name": "required", "age": ["required", "positive_integer"]}))
            .unwrap();
    assert_eq!(
        validator
            .validate_into::<User>(&json!({"name": "Jane", "age": "42"}))
            .unwrap(),
        User {
            name: "Jane".into(),
            age: 42
        }
    );
    assert!(matches!(
        validator.validate_into::<User>(&json!({"age": "42"})),
        Err(livr_core::ValidateIntoError::Invalid(_))
    ));
}

// Schemas and registries can come from storage that users write to: a deep
// or exponential one is a schema error, not a crash
#[test]
fn deep_and_huge_schemas_are_schema_errors() {
    // A chain of 3000 aliases, each using the next one
    let mut registry = Registry::default();
    for i in 0..3000 {
        registry
            .register_alias(Alias::new(format!("a{i}"), json!([format!("a{}", i + 1)])))
            .unwrap();
    }
    registry
        .register_alias(Alias::new("a3000", json!("required")))
        .unwrap();
    let error = Validator::builder(&json!({"x": "a0"}))
        .registry(&registry)
        .build()
        .unwrap_err();
    assert_eq!(
        error.message(),
        format!(
            "rules are nested more than {} levels deep",
            livr_core::MAX_DEPTH
        )
    );

    // Aliases that use the next one twice: 2^30 rules
    let mut registry = Registry::default();
    for i in 0..30 {
        let next = format!("b{}", i + 1);
        registry
            .register_alias(Alias::new(format!("b{i}"), json!([next, next])))
            .unwrap();
    }
    registry
        .register_alias(Alias::new("b30", json!("required")))
        .unwrap();
    let error = Validator::builder(&json!({"x": "b0"}))
        .registry(&registry)
        .build()
        .unwrap_err();
    assert_eq!(
        error.message(),
        format!(
            "the schema has more than {} rules, aliases expanded",
            livr_core::MAX_RULES
        )
    );

    // 2000 nested `or`, built in code: serde_json would not parse it
    // (`json!` would copy the value at every level)
    let rule =
        |name: &str, args: Value| Value::Object([(name.to_owned(), args)].into_iter().collect());
    let mut rules = json!("required");
    for _ in 0..2000 {
        rules = rule("or", Value::Array(vec![rules]));
    }
    let schema = rule("x", rules);
    let error = Validator::new(&schema).unwrap_err();
    assert!(error.message().starts_with("rules are nested"), "{error}");
    drop_deep(schema);

    // As deep as allowed is fine
    let mut rules = json!("required");
    for _ in 0..livr_core::MAX_DEPTH - 1 {
        rules = rule("nested_object", rule("a", rules));
    }
    assert!(Validator::new(&rule("x", rules)).is_ok());
}

#[test]
fn alias_has_getters() {
    let alias = Alias::new("adult", json!({"min_number": 18})).with_error("WRONG_AGE");
    assert_eq!(alias.name(), "adult");
    assert_eq!(alias.rules(), &json!({"min_number": 18}));
    assert_eq!(alias.error(), Some("WRONG_AGE"));
    assert_eq!(Alias::new("a", json!("required")).error(), None);
}

#[test]
fn codes_hash_like_their_strings() {
    use std::collections::HashSet;
    let codes: HashSet<livr_core::Code> = [
        livr_core::Code::from("REQUIRED"),
        livr_core::Code::from("REQUIRED".to_owned()),
        livr_core::Code::from("TOO_LONG"),
    ]
    .into_iter()
    .collect();
    assert_eq!(codes.len(), 2);
}

// The message of an error does not repeat its source: error reporters print
// the whole chain
#[test]
fn validate_into_errors_chain_their_source() {
    use std::error::Error;
    let validator = Validator::new(&json!({"age": ["required", "positive_integer"]})).unwrap();
    let error = validator
        .validate_into::<serde_json::Value>(&json!({}))
        .unwrap_err();
    assert_eq!(error.to_string(), "invalid data");
    assert_eq!(error.source().unwrap().to_string(), r#"{"age":"REQUIRED"}"#);
    let error = validator
        .validate_into::<String>(&json!({"age": 5}))
        .unwrap_err();
    assert_eq!(error.to_string(), "the output does not fit the type");
    assert!(error.source().is_some());
}

#[test]
fn validators_and_registries_are_unwind_safe() {
    fn assert_unwind_safe<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
    assert_unwind_safe::<Validator>();
    assert_unwind_safe::<Registry>();
    // With a custom rule inside, a panic can be caught around validation
    let mut registry = Registry::default();
    registry.register_rule(
        "boom",
        rule_builder(|_args, _ctx| {
            Ok(rule_fn(|value, _ctx| {
                assert!(value != "boom", "boom");
                Outcome::Valid
            }))
        }),
    );
    let validator = Validator::builder(&json!({"x": "boom"}))
        .registry(&registry)
        .build()
        .unwrap();
    let result = std::panic::catch_unwind(|| validator.validate(&json!({"x": "boom"})));
    assert!(result.is_err());
    assert!(validator.validate(&json!({"x": "ok"})).is_ok());
}

// Auto-trim walks the data without recursion: data built in code can be
// far deeper than serde_json parses
#[test]
fn auto_trim_handles_deep_data() {
    let validator = Validator::builder(&json!({"a": "string", "deep": "not_empty_list"}))
        .auto_trim(true)
        .build()
        .unwrap();
    let mut deep = json!(" x ");
    for _ in 0..100_000 {
        deep = Value::Array(vec![deep]);
    }
    let mut data = Map::new();
    data.insert("a".to_owned(), json!(" y "));
    data.insert("deep".to_owned(), deep);
    // Owned: the borrowed path copies the output with `Value::clone`, which
    // recurses like the drop of serde_json
    let output = validator.validate_owned(Value::Object(data)).unwrap();
    let Value::Object(mut output) = output else {
        panic!("an object")
    };
    assert_eq!(output["a"], "y");
    let mut value = output.remove("deep").unwrap();
    while let Value::Array(mut items) = value {
        value = items.pop().unwrap();
    }
    assert_eq!(value, "x");
}

// Takes a value apart without recursion: the drop of serde_json values is
// recursive, and would overflow the stack of a test
fn drop_deep(value: Value) {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(items) => stack.extend(items),
            Value::Object(fields) => stack.extend(fields.into_iter().map(|(_, value)| value)),
            _ => {}
        }
    }
}
