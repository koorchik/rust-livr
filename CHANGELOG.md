# Changelog

## 0.1.0 (unreleased)

- The rules of the LIVR specification 2.0: all 70 cases of the test suite
  pass, also with camelCase rule names, owned data and typed schemas.
- `Validator`: schemas known at run time, with custom rules, aliases and
  auto-trim; `validate` (borrowed data) and `validate_owned` (owned data,
  values moved instead of copied).
- `schema!`: schemas known at compile time, checked by the compiler, with
  the type of the output inferred from the rules. Its rules are compiled
  ahead of time into direct calls.
- `livr::Schema`: the trait of the types of `schema!`, for code generic over
  schemas.
- `livr-wasm`: the validator for JavaScript, compiled to WebAssembly.

### Before the release

Fixes:
- `schema!`: a selector field with an empty list of rules (`"type": []`) no
  longer gives a tagged enum; the field is dropped from the output, so the
  type is a map, like the output of the validator.
- `schema!`: a key used twice in a schema or in a JSON object is a compile
  error (JSON keeps the last one, Rust would have had two fields).
- `schema!`: a name used by both a custom rule and an alias, or by two of
  them, also as camelCase forms of each other, is a compile error (the types
  followed one rule, the run time another).
- `schema!`: the same `as Name` twice, or the name of the schema, is a
  compile error. Generated names avoid the names of `as Name`.
- `schema!`: values and keys that are no Rust names (`"self"`, `"²"`, `"a²"`)
  give valid names instead of errors or a panic of the macro; the generated
  code works under `#![no_implicit_prelude]`.
- `like` matches like JavaScript without the `u` flag: `.` stops at line
  terminators; with `i`, ASCII letters match only ASCII letters (`k` does not
  match the Kelvin sign, `s` not the long s); `\s` is the whitespace of
  JavaScript (U+FEFF, not U+0085); escaped letters without a meaning are
  literals (`\z`, `\p{L}`); `\0`, `\cX`, and braces that are not quantifiers
  (`a{`) work. Inline flags such as `(?i)` are a schema error instead of
  having their Rust meaning. The `regex` crate no longer needs its
  `unicode-perl` tables.
- A schema is limited to 64 levels of nested rules (`MAX_DEPTH`) and 100,000
  rules with aliases expanded (`MAX_RULES`): deep alias chains, aliases that
  double at every level, and deeply nested `or` were a stack overflow or took
  gigabytes. Auto-trim walks the data without recursion.

Performance:
- Typed code reads the data once, with a `match` on the key per field.
- `Validator` walks the sorted schema and the data together.
- `validate_owned` with auto-trim trims in place; owned lists are validated
  in place.
- `to_lc` and `to_uc` allocate nothing for strings that do not change.

API, before it becomes semver:
- `Alias` has private fields, with `Alias::new`, `with_error` and the getters
  `name`, `rules`, `error`; `schema!` uses the constructor.
- `Schema::FIELDS` and `SchemaField` (`name`, `is_list`): the fields of a
  schema and which are lists, for extractors of forms and query strings.
- `ValidateIntoError` is `#[non_exhaustive]`, and its message no longer
  repeats its source.
- `Outcome` is `#[must_use]`, `Code` implements `Hash`, `Validator` and
  `Registry` are unwind safe, the types of the compiled rules implement
  `Debug`.
- Renamed rule types: `rules::Default` is `DefaultRule`, `rules::Eq` is
  `EqRule`.
- Hidden from the documentation, not part of the public API: the module
  `rules`, `TypedObject`, `CompileCtx::lazy` and the helpers of `util` except
  `arg`, `number_arg`, `string_arg` and `is_no_value`.
- `livr` forwards the `preserve_order` feature; `serde/derive` comes with the
  `macros` feature.

Packaging:
- Every published crate has the LICENSE file and a README; `livr-core` and
  `livr-macros` are pinned to the exact version of `livr`.
- The LIVR test suite (Artistic-2.0) is not in the `livr-core` package; its
  tests skip it.
- The documentation of `schema!` is on docs.rs.
