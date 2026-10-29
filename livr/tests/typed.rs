//! schema!: the types inferred from the rules, and validation with them.

use livr::{CompileCtx, Errors, Outcome, SchemaError, Value};
use serde_json::json;

// Asserts the type of an expression at compile time
fn has_type<T>(_: &T) {}

livr::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct User {
        name: ["required", {"max_length": 20}],
        email: ["required", "email", "to_lc"],
        age: "positive_integer",
        score: {"number_between": [0, 10]},
        role: ["required", {"one_of": ["admin", "user", "super-user"]}],
        level: {"one_of": [1, 2, 3]},
        nick: "trim",
        tags: {"list_of": ["required", "string"]},
        codes: {"list_of": "integer"},
        address: ["required", {"nested_object": {
            city: ["required", "string"],
            zip: ["required", {"like": "^\\d{5}$"}],
            "street-name": "string",
        }}],
        phones: {"list_of_objects": {
            number: "required",
            kind: {"one_of": ["home", "work"]},
        }} as Phone,
        settings: "any_object",
        contact: {"or": [["required", "email"], ["required", "positive_integer"]]},
        limit: ["integer", {"default": 10}],
        "type": "string",
        meta: "trim",
    }
}

#[test]
fn infers_types_from_rules() {
    let user = User::validate(&json!({
        "name": "Jane",
        "email": "JANE@EXAMPLE.COM",
        "age": "42",
        "score": "7.5",
        "role": "super-user",
        "level": "2",
        "nick": " jj ",
        "tags": ["a", "b"],
        "codes": [1, "", 3],
        "address": {"city": "Kyiv", "zip": "01001", "street-name": "Main", "extra": 1},
        "phones": [{"number": "555", "kind": "home"}],
        "settings": {"theme": "dark"},
        "contact": "17",
        "type": 5,
    }))
    .unwrap();

    has_type::<String>(&user.name);
    has_type::<String>(&user.email);
    has_type::<Option<i64>>(&user.age);
    has_type::<Option<f64>>(&user.score);
    has_type::<UserRole>(&user.role);
    has_type::<Option<i64>>(&user.level);
    has_type::<Option<Value>>(&user.nick);
    has_type::<Option<Vec<String>>>(&user.tags);
    has_type::<Option<Vec<Option<i64>>>>(&user.codes);
    has_type::<UserAddress>(&user.address);
    has_type::<Option<String>>(&user.address.street_name);
    has_type::<Option<Vec<Phone>>>(&user.phones);
    has_type::<Option<PhoneKind>>(&user.phones.as_ref().unwrap()[0].kind);
    has_type::<Option<livr::Map<String, Value>>>(&user.settings);
    has_type::<UserContact>(&user.contact);
    has_type::<i64>(&user.limit);
    has_type::<Option<String>>(&user.r#type);

    assert_eq!(user.email, "jane@example.com");
    assert_eq!(user.age, Some(42));
    assert_eq!(user.score, Some(7.5));
    assert_eq!(user.role, UserRole::SuperUser);
    assert_eq!(user.role.as_str(), "super-user");
    assert_eq!(user.level, Some(2));
    assert_eq!(user.nick, Some(json!("jj")));
    assert_eq!(user.codes, Some(vec![Some(1), None, Some(3)]));
    assert_eq!(user.address.zip, "01001");
    assert_eq!(user.contact, UserContact::Integer(17));
    assert_eq!(user.limit, 10);
    assert_eq!(user.r#type.as_deref(), Some("5"));
    assert_eq!(user.meta, None);

    // The output serializes back to the output of the validator
    let output = serde_json::to_value(&user).unwrap();
    assert_eq!(output, User::validator().validate(&json!({
        "name": "Jane", "email": "JANE@EXAMPLE.COM", "age": "42", "score": "7.5", "role": "super-user",
        "level": "2", "nick": " jj ", "tags": ["a", "b"], "codes": [1, null, 3],
        "address": {"city": "Kyiv", "zip": "01001", "street-name": "Main"},
        "phones": [{"number": "555", "kind": "home"}], "settings": {"theme": "dark"}, "contact": "17", "type": 5,
    })).unwrap());
}

#[test]
fn returns_the_errors_of_the_validator() {
    let errors = User::validate(
        &json!({"name": "", "email": "x", "role": "guest", "address": {"city": "Kyiv"}}),
    )
    .unwrap_err();
    assert_eq!(
        serde_json::to_value(errors).unwrap(),
        json!({
            "name": "REQUIRED",
            "email": "WRONG_EMAIL",
            "role": "NOT_ALLOWED_VALUE",
            "address": {"zip": "REQUIRED"},
            "contact": "REQUIRED",
        })
    );
    assert_eq!(User::validate(&json!([])), Err(Errors::Format));
}

livr::schema! {
    #[derive(Debug, PartialEq)]
    #[alias(adult_age = ["positive_integer", {"min_number": 18}], error = "WRONG_AGE")]
    #[rule(starts_with = starts_with, output = String)]
    struct Person {
        age: ["required", "adult_age"],
        code: {"starts_with": "X-"},
        event: ["required", {"variable_object": ["kind", {
            click: {kind: "required", x: ["required", "integer"]},
            scroll: {kind: ["required", "string"], up: "integer"},
        }]}],
    }
}

fn starts_with(
    args: &[Value],
    _ctx: &mut CompileCtx<'_>,
) -> Result<impl livr::Rule + use<>, SchemaError> {
    let prefix = livr::util::string_arg(args, 0, "starts_with")?;
    Ok(livr::rule_fn(move |value, _ctx| match value {
        Value::String(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
        _ => Outcome::Valid,
    }))
}

#[test]
fn uses_aliases_custom_rules_and_variants() {
    let person = Person::validate(
        &json!({"age": "20", "code": "X-1", "event": {"kind": "click", "x": "5"}}),
    )
    .unwrap();
    has_type::<i64>(&person.age);
    has_type::<Option<String>>(&person.code);
    has_type::<PersonEvent>(&person.event);
    assert_eq!(person.event, PersonEvent::Click(PersonEventClick { x: 5 }));
    assert_eq!(
        serde_json::to_value(&person.event).unwrap(),
        json!({"kind": "click", "x": 5})
    );

    let errors =
        Person::validate(&json!({"age": 12, "code": "Y", "event": {"kind": "drag"}})).unwrap_err();
    assert_eq!(
        serde_json::to_value(errors).unwrap(),
        json!({"age": "WRONG_AGE", "code": "WRONG_PREFIX", "event": "FORMAT_ERROR"})
    );
}

livr::schema!(pub struct FromFile = "tests/schemas/order.json");

#[test]
fn reads_schemas_from_files() {
    let order =
        FromFile::validate_owned(json!({"id": "7", "items": [{"sku": "A", "quantity": 2}]}))
            .unwrap();
    has_type::<i64>(&order.id);
    has_type::<Vec<FromFileItemsItem>>(&order.items);
    assert_eq!(order.items[0].quantity, 2);
}

// Rules compiled ahead of time next to rules that need the registry at run
// time: custom rules and aliases inside `or` and `list_of`
livr::schema! {
    #[derive(Debug, PartialEq)]
    #[alias(adult_age = ["positive_integer", {"min_number": 18}], error = "WRONG_AGE")]
    #[rule(starts_with = starts_with, output = String)]
    struct Mixed {
        role: ["required", {"one_of": ["admin", "user"]}],
        id: ["required", {"or": [[{"starts_with": "X-"}], ["adult_age"]]}],
        ages: {"list_of": ["required", "adult_age"]},
    }
}

#[test]
fn mixes_rules_compiled_ahead_of_time_with_registry_rules() {
    for data in [
        json!({"role": "admin", "id": "X-1", "ages": ["20", 30]}),
        json!({"role": "user", "id": "18", "ages": []}),
    ] {
        let mixed = Mixed::validate(&data).unwrap();
        assert_eq!(Mixed::validate_owned(data).unwrap(), mixed);
    }
    let data = json!({"role": "guest", "id": "Y-1", "ages": ["20", 12]});
    let errors = json!({
        "role": "NOT_ALLOWED_VALUE",
        "id": "WRONG_AGE",
        "ages": [null, "WRONG_AGE"],
    });
    assert_eq!(
        serde_json::to_value(Mixed::validate(&data).unwrap_err()).unwrap(),
        errors
    );
    assert_eq!(
        serde_json::to_value(Mixed::validate_owned(data).unwrap_err()).unwrap(),
        errors
    );
}

// A selector field with an empty list of rules is dropped from the output,
// so the variants cannot be told apart: the type is a map, like the output
// of the validator
livr::schema! {
    #[derive(Debug)]
    struct EmptySelector {
        v: ["required", {"variable_object": ["type", {"a": {"type": [], "x": "required"}}]}],
        list: {"list_of_different_objects": ["type", {"a": {"type": [], "x": "required"}}]},
    }
}

#[test]
fn an_empty_selector_rule_list_keeps_the_output_of_the_validator() {
    let data = json!({"v": {"type": "a", "x": 1}, "list": [{"type": "a", "x": 2}]});
    let expected = json!({"v": {"x": 1}, "list": [{"x": 2}]});
    assert_eq!(
        EmptySelector::validator().validate(&data),
        Ok(expected.clone())
    );
    let typed = EmptySelector::validate(&data).unwrap();
    has_type::<livr::Map<String, Value>>(&typed.v);
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);
    let typed = EmptySelector::validate_owned(data).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);
}

livr::schema! {
    pub struct Search {
        query: ["required", "string"],
        ignored: [],
        tags: {"list_of": "string"},
        "page-size": ["positive_integer", {"default": 20}],
        items: ["required", {"list_of_objects": {id: "required"}}],
        events: {"list_of_different_objects": ["kind", {a: {kind: "required"}}]},
        checked: "not_empty_list",
        either: {"or": [{"list_of": "integer"}, {"list_of": "string"}]},
    }
}

// What livr-axum needs to prepare a query string or a form: the fields, and
// which are lists
#[test]
fn schema_fields_are_listed_in_schema_order() {
    use livr::{Schema, SchemaField};
    let fields: Vec<(&str, bool)> = <Search as Schema>::FIELDS
        .iter()
        .map(|field| (field.name(), field.is_list()))
        .collect();
    assert_eq!(
        fields,
        [
            ("query", false),
            ("tags", true),
            ("page-size", false),
            ("items", true),
            ("events", true),
            ("checked", true),
            // An enum of two list types, not a `Vec`
            ("either", false),
        ]
    );
    assert_eq!(Search::FIELDS, <Search as Schema>::FIELDS);
    assert_eq!(User::FIELDS.len(), 16);
    assert_eq!(User::FIELDS[7], SchemaField::new("tags", true));
}

// Values and keys that are no Rust names
livr::schema! {
    #[derive(Debug, PartialEq)]
    struct Names {
        kind: {"one_of": ["self", "self_", "²", "Self"]},
        event: {"variable_object": ["type", {"self": {"type": "required"}, "²": {"type": "required"}}]},
        "a²": {"nested_object": {"b": "string"}},
    }
}

#[test]
fn names_that_are_no_identifiers_are_escaped() {
    let names =
        Names::validate(&json!({"kind": "self", "event": {"type": "self"}, "a²": {"b": "x"}}))
            .unwrap();
    assert_eq!(names.kind, Some(NamesKind::Self_));
    assert_eq!(NamesKind::Self_2.as_str(), "self_");
    assert_eq!(NamesKind::V.as_str(), "²");
    assert_eq!(NamesKind::Self_3.as_str(), "Self");
    assert!(matches!(names.event, Some(NamesEvent::Self_(_))));
    has_type::<Option<NamesA>>(&names.a_);
    assert_eq!(
        serde_json::to_value(&names).unwrap(),
        json!({"kind": "self", "event": {"type": "self"}, "a²": {"b": "x"}})
    );
}

// The generated code names everything by its full path
mod no_prelude {
    #![no_implicit_prelude]

    ::livr::schema! {
        #[alias(adult = ["positive_integer", {"min_number": 18}])]
        pub struct Order {
            id: ["required", "positive_integer"],
            age: "adult",
            kind: {"one_of": ["a", "b"]},
            item: {"nested_object": {sku: "required"}},
            items: {"list_of_objects": {sku: "required"}},
            codes: {"list_of": "integer"},
            event: {"variable_object": ["type", {x: {"type": "required"}}]},
            either: {"or": ["integer", {"nested_object": {a: "string"}}]},
            note: ["trim", {"default": ""}],
        }
    }
}

#[test]
fn generated_code_works_without_the_prelude() {
    let order =
        no_prelude::Order::validate(&json!({"id": "1", "age": 20, "items": [{"sku": "A"}]}))
            .unwrap();
    assert_eq!(order.id, 1);
    assert!(no_prelude::Order::validate_owned(json!({"id": 0})).is_err());
}
