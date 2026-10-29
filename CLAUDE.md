# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust implementation of the LIVR specification 2.0 (https://livr-spec.org). Performance matters (measure changes on hot paths with the benchmark), but a convenient public API comes first: don't trade ergonomics for a small speedup. Big measured wins that keep the API are welcome. Type inference of `schema!` must stay sound: for every valid data, the output fits the generated types.

## Commands

```bash
cargo test --workspace                          # everything: suite, camelCase, typed suite, API, compile-fail, doctests (README in livr-typed-suite)
cargo test -p livr-core --features preserve_order   # the engine with serde_json objects in insertion order
cargo test -p livr-core --test test_suite       # the LIVR test suite only
TRYBUILD=overwrite cargo +1.88.0 test -p livr --test ui   # update the expected compile errors (livr/tests/ui/*.stderr); CI checks them on 1.88 only
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
cargo publish --workspace --dry-run --allow-dirty  # the published crates build from their packages
taskset -c 9 cargo bench -p livr                # benchmark (best of 5 runs); pin it, other processes disturb it
livr-wasm/build.sh && taskset -c 9 node bench-js/bench.mjs   # WebAssembly build and the JavaScript comparison
cp -r ../LIVR/test_suite livr-core/tests/            # refresh the copy of the LIVR test suite (change it in the specification first)
```

## Architecture

- `livr` - the published facade: re-exports `livr-core` and `schema!`; `__private` has what generated code uses (`step`, typed field validation)
- `livr-core` - the engine
  - `compile.rs` - `CompileCtx`, `Chain` (rules of a field), `ObjectValidator` (names and rules of all fields in one allocation each), aliases, `run_items`; the limits `MAX_DEPTH` (nesting) and `MAX_RULES` (aliases expanded), since schemas and registries can come from storage
  - `rule.rs` - the rule contract: `Outcome`, `Ctx` (other fields: object, single value, none), `Rule`, `RuleBuilder`
  - `registry.rs` - `Registry` (no global mutable state; `Registry::builtin()` is a frozen `OnceLock`), `Alias`, camelCase names
  - `validator.rs` - `Validator` (`Arc`, immutable, `Send + Sync`), `validate`, `validate_owned`, `validate_into`
  - `typed.rs` - `FromLivr` (owned), `FromLivrRef` (borrowed), `TypedObject`, `Schema` (the main type of `schema!`), `ConvertError` (becomes `FORMAT_ERROR`, never a panic)
  - `rules/<group>/<rule>.rs` - one rule per file; `rules/mod.rs` - the `built_in_rules!` list: the `CompiledRule` enum (static dispatch) and the name → builder table
- `livr-macros` - `schema!`: `parse.rs` (JSON in tokens, `#[rule]`, `#[alias]`), `infer.rs` (type inference), `lib.rs` (checks the schema with livr-core at compile time, generates the code), `aot.rs` (the rules of a field as code)
- `livr-typed-suite` - build.rs generates a `schema!` and a test for every case of the test suite; also runs the examples of README.md as doctests (the README is outside the published crates)
- `livr-wasm` - wasm-bindgen wrapper (not published); `bench-js` - benchmark against js-validator-livr

Sibling repositories, checked out next to this one, depend on `livr` by path (`../rust-livr/livr`): `rust-livr-axum` (the `livr-axum` crate, generic over `livr::Schema`) and `rust-chista-examples` (a backend with services, sqlx and SeaORM). A change to the public API of `livr` can break them: run their tests too.

### Values: `Cow`

Compiled rules run on `Cow<Value>` (`Checked<'v> = Result<Cow<'v, Value>, ErrorTree>`):
- A borrowed value stays borrowed until a rule changes it; an owned one is moved through the rules. One implementation serves borrowed data (copying only the output) and owned data (copying nothing).
- A missing field is `Cow::Borrowed(&NULL)`, so an owned result means a rule gave it a value (`default`).
- Simple rules have `check(&Value, &Ctx) -> Outcome`; metarules have `run(Cow)` too.
- Owned values cannot be read as other fields, so objects whose rules read other fields (`equal_to_field`, custom rules: `reads_ctx`) are validated borrowed.

### Rule files

- One rule per file, the snake_case spec name as the file name, a public struct with `build(args, ctx)` and `check(&self, value, ctx)`. A rule file uses only `crate::rules::prelude`.
- No imports between rule files: repeat small code instead.
- Metarules use only `CompileCtx::compile_field` / `compile_object`.
- Expensive work goes to `build`: arguments, regular expressions (cached by pattern in `like`), lookup tables. `check` only checks.
- Rules skip empty values (`is_no_value`), except `required`, `not_empty` and `not_empty_list`.
- Non-primitive values are `FORMAT_ERROR` for string, numeric and special rules; modifiers leave them as they are.
- Invalid arguments are a `SchemaError` from `build` (`arg`, `number_arg`, `string_arg`).

### Semantics that differ from JavaScript

Strict, like dart-livr and the LIVR 3 draft, and listed in README.md (keep it in sync):
- decimal-only number strings (`parse_number`); booleans are not numbers; integers are `i64`;
- lengths in code points;
- exactly one rule name per rule object;
- `like` translates JavaScript (without the `u` flag) to the `regex` syntax with the same meaning: classes as code point ranges, case folded by the translation (ECMAScript `Canonicalize`, not Unicode folding), identity escapes as literals; what has no equivalent (look-around, backreferences, inline flags) is a `SchemaError`. Its test table is checked against Node.js.

### Types of `schema!`

`infer.rs` folds the rules of a field into `State { ty, present }`. Rules for soundness:
- modifiers keep an unknown type as `Value`, since objects pass through them;
- `trim`, `remove` and `leave_only` reset presence, since they can leave `""`;
- `variable_object` gives an enum only when every schema keeps the selector field.

The generated code validates fields itself (`TypedObject`) when the last rule is `nested_object`, `list_of_objects` or `list_of`; otherwise it runs the rules of the field and converts the output. The typed suite checks both paths against the test suite.

The rules of a field are compiled ahead of time (`aot.rs`): a function per field that calls each rule with `__private::step`, like `Chain::run` does. Rules without heap data are constants, the others are built once from their arguments in a `LazyLock`; metarules inside a chain, aliases and custom rules are compiled by the engine. A rule constructor used there (`MaxLength::new`) is part of the contract between the crates: keep it `const` and in sync with `build`.

## Release

1. Bump `version` in the root `Cargo.toml` and the exact pins `=x.y.z` of `livr-core` and `livr-macros` next to it: the generated code uses internals of livr-core, so the three crates are released together with the same version.
2. Add the version to `CHANGELOG.md`; run the commands above, including `cargo publish --workspace --dry-run`.
3. Commit, tag `vX.Y.Z`, push, and create a GitHub release from the CHANGELOG entry.
4. `cargo publish --workspace` publishes livr-core, livr-macros and livr in order and waits for the index (cargo 1.90 or newer).
5. Then `rust-livr-axum`: raise its `livr` version if it needs the new release, and publish it (see its CLAUDE.md).
6. The LIVR site ([koorchik/LIVR](https://github.com/koorchik/LIVR)) runs `conformance/rust` against `livr` from crates.io, with the requirement `livr = "0.1"`: a 0.1.x release shows up on the next deploy; for 0.2 update `conformance/rust/Cargo.toml` there. `NATIVE=1 LIVR_RUST_PATH="$PWD/../rust-livr" conformance/run.sh rust`, run in the LIVR checkout, tests this checkout instead (an absolute path; the Docker mode always uses crates.io).

## Code style

`cargo fmt`, clippy without warnings, `#![forbid(unsafe_code)]`, edition 2024, rust-version 1.88. Comments explain why, in plain sentences. Zero runtime dependencies beyond serde, serde_json and regex.
