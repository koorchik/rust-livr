# Changelog

## 0.2.2 (2026-11-08)

- Changed, as LIVR 2.5 specifies now: `url` rejects a dot after the host
  (`https://example.com.`), host labels longer than 63 characters and ports
  above 65535, and accepts one-digit ports (`https://example.com:8`). The
  rest of its grammar is the same: Unicode hosts, user info, `localhost` and
  IPv4 addresses pass as before. The copy of the 2.5 test suite has the new
  cases.

## 0.2.1 (2026-11-07)

- Docs: LIVR 2.5 is released, not a draft. The README lists all the rules,
  of LIVR 2.0 and of LIVR 2.5.

## 0.2.0 (2026-11-07)

- The rules of the LIVR 2.5 draft: `boolean`, `not_one_of`, `max_decimals`,
  `digits`, `starts_with`, `ends_with`, `contains`, `iso_datetime`,
  `iso_time`, `required_if`, `when`, `values`, `keys`, `any`, `list_length`,
  `unique`, `split`, the formats `uuid`, `ip`, `cidr`, `hostname`, `base64`,
  `hex`, `digest`, `mongo_id`, `hex_color`, `mac`, `e164`, `semver`, `jwt`, and
  the checksums `luhn`, `credit_card`, `iban`. `iso_date` takes bounds,
  `{"min": ..., "max": ...}` (dates, `today`, `yesterday`, `tomorrow`). All
  cases of the 2.5 test suite pass, its schema errors included, also with
  camelCase names and as typed schemas, which infer `bool` for `boolean` and
  maps for `values` and `keys`.
- Arguments of the new rules as the draft defines them: the arguments after
  the ones a rule takes are ignored (`{"split": [",", ";"]}`), a number and a
  string with a number are the same (`{"max_decimals": "2"}`, `{"split": 1}`),
  and `{"rule": null}` is one `null` argument, a schema error for them
  (`{"boolean": null}`). The 2.0 rules and custom rules keep their argument
  handling (`{"default": null}` sets `null`, `{"iso_date": null}` is
  `"iso_date"`).
- `ValidatorBuilder::now` and `ValidatorBuilder::clock`: the current time of
  `today`, `yesterday` and `tomorrow` (UTC). `schema!` takes `#[clock(path)]`,
  a `fn() -> SystemTime`, for the same.
- Changed, as LIVR 2.5 fixes 2.0: the rules of aliases and of `or` see the
  object of the field (`equal_to_field` in an alias compares with a sibling of
  the field), the rules of `list_of` items see no other fields. `iso_date`
  with a first argument that is not an object (`{"iso_date": "today"}`,
  ignored by LIVR 2.0) is a schema error. A custom rule registered with a new
  standard name (`starts_with`, `uuid`) replaces the built-in one.
- The copy of the 2.0 test suite has the new cases (`like` with `^` and `$`,
  integers in `decimal`, `null` kept in the output, `list_of*` on an object, a
  modifier in `or`, `iso_date` in the proleptic Gregorian calendar). They
  passed without code changes.

## 0.1.1 (2026-10-29)

- One runtime dependency less: numbers are written as strings like
  JavaScript's `String()` (`10.0` as `"10"`, `1e21` as `"1e+21"`) with the
  standard library instead of `ryu-js`. The output is the same; a test checks
  it against `ryu-js` on random numbers, and the formatting was compared with
  Node.js on a million of them.

## 0.1.0 (2026-10-29)

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
