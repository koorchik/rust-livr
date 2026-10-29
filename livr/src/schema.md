A LIVR schema known at compile time, with the Rust types of its output
inferred from the rules.

The macro checks the schema with the same engine as the run time: an unknown
rule, invalid arguments or a recursive alias is a compile error on its field.
It then generates a struct, and the structs and enums of nested objects,
lists and `one_of` values.

```
livr::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct SignUp {
        name: ["trim", "required", {"max_length": 50}],
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

let user = SignUp::validate(&serde_json::json!({
    "name": " Jane ", "email": "JANE@EXAMPLE.COM", "age": "42",
    "address": {"city": "Kyiv", "zip": "01001"},
}))?;
assert_eq!((user.name.as_str(), user.age), ("Jane", 42));
assert_eq!(user.address, SignUpAddress { city: "Kyiv".into(), zip: "01001".into() });
assert_eq!(user.gender, None);
# Ok::<(), livr::Errors>(())
```

# Syntax

```text
livr::schema! {
    #[derive(...)]                                // attributes for every generated type
    #[rule(name = path::to::builder, output = Type, present)]   // custom rules
    #[alias(name = rules, error = "CODE")]        // aliases
    pub struct Name {
        key: rules,                               // a rule, or a list of rules
        "key-with-dashes": rules,                 // a key that is no identifier
        key: rules as TypeName,                   // the name of the type of the field
    }
}

livr::schema!(pub struct Name = "schemas/name.json");   // the schema in a JSON file
```

- **Rules** are JSON: strings, numbers, `true`, `false`, `null`, lists and
  objects. Keys of objects can be identifiers (`{city: "required"}`) or
  strings. A key used twice is a compile error, where JSON would keep the
  last one.
- **`#[rule(name = builder, ...)]`** registers a custom rule: `builder` is a
  [rule builder](crate::RuleBuilder), a function
  `fn(&[Value], &mut CompileCtx<'_>) -> Result<impl Rule, SchemaError>`.
  `output = Type` is the type of the value after the rule (the type does
  not change when it is left out; `Type` implements [`FromLivr`](crate::FromLivr)
  and [`FromLivrRef`](crate::FromLivrRef)); `present` says the rule makes the
  field present, like `required`.
- **`#[alias(name = rules, error = "CODE")]`** registers an alias, with an
  optional error code that replaces the errors of its rules.
- A custom rule and an alias cannot have the same name, nor names equal in
  camelCase (`foo_bar` and `fooBar`): every rule is also registered under
  its camelCase name.
- **`= "file.json"`** reads the schema from a file, relative to the
  directory of `Cargo.toml` (`CARGO_MANIFEST_DIR`); the crate is rebuilt when
  the file changes.
- **`as TypeName`** gives a name to the first type generated for the field;
  the types inside it are named after it. A name given twice is a compile
  error.

```
use livr::{CompileCtx, Outcome, Rule, SchemaError, Value, rule_fn, util::string_arg};

fn starts_with(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<impl Rule + use<>, SchemaError> {
    let prefix = string_arg(args, 0, "starts_with")?;
    Ok(rule_fn(move |value, _ctx| match value.as_str() {
        Some(str) if !str.starts_with(&prefix) => Outcome::code("WRONG_PREFIX"),
        _ => Outcome::Valid,
    }))
}

livr::schema! {
    #[derive(Debug)]
    #[rule(starts_with = starts_with, output = String)]
    #[alias(adult_age = ["positive_integer", {"min_number": 18}], error = "WRONG_AGE")]
    pub struct Person {
        code: ["required", {"starts_with": "X-"}],   // String
        age: ["required", "adult_age"],              // i64
        phones: {"list_of_objects": {number: "required"}} as Phone,   // Option<Vec<Phone>>
    }
}

let errors = Person::validate(&serde_json::json!({"code": "Y-1", "age": 12})).unwrap_err();
assert_eq!(errors.to_string(), r#"{"age":"WRONG_AGE","code":"WRONG_PREFIX"}"#);
```

# What is generated

For `pub struct Name`:

- `Name::validate(&Value)` borrows the data, `Name::validate_owned(Value)`
  moves its values instead of copying them; both return
  `Result<Name, Errors>`.
- `Name::SCHEMA`: the schema as JSON (without custom rules and aliases), for
  a frontend that validates with the same rules.
- `Name::validator()`: a [`Validator`](crate::Validator) of the schema, for
  an output that is not typed.
- `Name::FIELDS`: the fields of the output in the order of the schema, as
  [`SchemaField`](crate::SchemaField)s: their key and whether they are lists.
- An implementation of [`Schema`](crate::Schema) with all of these.
- Every generated type implements `serde::Serialize` and serializes back to
  the output of the schema; the attributes before the struct (like
  `#[derive(Debug)]`) apply to all of them.

```
livr::schema! {
    pub struct Search {
        query: ["required", "string"],
        tags: {"list_of": "string"},
    }
}

// A query string has no lists: `?tags=a` must become `["a"]`
let fields: Vec<_> = Search::FIELDS.iter().map(|field| (field.name(), field.is_list())).collect();
assert_eq!(fields, [("query", false), ("tags", true)]);
```

# Types

The rules of a field are folded into a type and a presence, in order:

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
| `not_empty_list` | `Vec<Value>`, unless a list type is known |
| `variable_object`, `list_of_different_objects` | an enum tagged by the selector field, when every schema has the selector field and keeps its value; `Map` otherwise |
| `or` | the type of the rule sets when it is the same, else an untagged enum |
| `any_object` | `Map<String, Value>` |
| `trim`, `to_lc`, `to_uc`, `remove`, `leave_only` | `String`; `Value` when the earlier type is unknown, since objects pass through them unchanged |
| `default` | the earlier type |
| a custom rule | its `output`, else the earlier type |
| no rule that tells the type | `Value` |

The types are sound: for every valid data, the output fits them. A custom
rule that outputs another type than it declares cannot panic the
conversion: the field gets `FORMAT_ERROR`.

# Presence

A field is `T` when a rule makes it present, and `Option<T>` otherwise. An
empty value (missing, `null` or `""`) of an optional field is `None`.

- `required`, `not_empty_list`, a `default` that is not empty, and custom
  rules declared `present` make a field present.
- `trim`, `remove` and `leave_only` can leave an empty string: after them a
  field is optional again, until another `required`.
- `or` is present when all its rule sets are.
- Items of `list_of` are present when their rules make them present
  (`Vec<String>` for `["required", "string"]`, `Vec<Option<i64>>` for
  `"integer"`); items of `list_of_objects` always are.
- A field with an empty list of rules (`key: []`) is not in the output, nor
  in the struct.

# Names

- **Fields** are the keys in snake_case: `firstName` and `first-name` become
  `first_name`, keywords become raw identifiers (`r#type`), `self` becomes
  `self_`, and a name that two keys give gets a number (`first_name_2`).
  The output keeps the keys of the schema.
- **Types** are named after their path, in PascalCase: the parent type, then
  the key of the field (`SignUp` and `address` give `SignUpAddress`). The
  items of a list add `Item` (`OrderItemsItem`), the variants of a
  `variable_object` add their selector value (`PersonEventClick`), the rule sets of
  an `or` their position (`UserContact1`). A name taken in the same schema
  gets a number (`SignUpAddress2`).
- **Variants** of enums are the values in PascalCase: `"super-user"` becomes
  `SuperUser`, `"1"` becomes `V1`, `""` becomes `Empty`, `"self"` becomes
  `Self_`; characters that cannot be in a name are left out.

Generated names can meet other types of the module: `struct A` with a field
`b_c`, and `struct AB` with a field `c`, both generate `ABC`. Two `schema!`
in one module, or a type of your own named like a generated one, are then a
compile error about a type defined twice: name the type of the field with
`as Name`, or put the schemas in their own modules.
