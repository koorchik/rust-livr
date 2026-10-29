# Benchmarks

The scenarios of the LIVR benchmarks (js-validator-livr, dart-livr):

- **Simple:** a 5-field form.
- **Nested:** two levels of objects, a list, and `like ^\d{5}$`.
- **Static:** the validator is built once. **Dynamic:** a new validator is built for every validation.
- **Invalid:** every field fails.
- **JSON string:** the data arrives as JSON, as in a request body. It is parsed, then validated.

Each scenario runs over a pool of 1000 inputs: 0.5 s of warm-up, then the best of 5 runs of 0.5 s.

- Rust: `livr/benches/validate.rs`, run with `cargo bench -p livr`.
- JavaScript and WebAssembly: `bench-js/bench.mjs`. It also checks that livr-wasm gives the same outputs and errors as js-validator-livr.

## Results

| | |
|---|---|
| CPU | i9-13900H, all runs pinned to CPU 9 (one P-core thread) with `taskset` |
| Load average | 1.4–2.9 for the Rust column (other processes ran on the machine), 2–3 for the others |
| Rust | 1.99.1, the bench profile of the workspace (fat LTO, one codegen unit) |
| Node.js | v26.7.0 |
| js-validator-livr | 2.12.1 |

| Scenario | livr (Rust) | js-validator-livr | livr-wasm, JSON both ways | livr-wasm, objects |
|---|---|---|---|---|
| simple, static | 4.01 M/s | 4.26 M/s | 339 K/s | 234 K/s |
| simple, dynamic | 1.97 M/s | 1.77 M/s | 128 K/s | 93 K/s |
| nested, static | 2.48 M/s | 2.42 M/s | 230 K/s | 159 K/s |
| nested, dynamic | 1.14 M/s | 941 K/s | 87 K/s | 60 K/s |
| invalid, static | 4.71 M/s | 4.36 M/s | 361 K/s | 143 K/s |
| simple, typed (`schema!`) | 9.32 M/s | | | |
| nested, typed (`schema!`) | 4.64 M/s | | | |
| simple, static, JSON string | 1.90 M/s | 1.53 M/s | 353 K/s | 304 K/s |
| nested, static, JSON string | 1.50 M/s | 928 K/s | 247 K/s | 210 K/s |
| simple, typed, JSON string | 2.97 M/s | | | |
| nested, typed, JSON string | 1.88 M/s | | | |

The Rust column is the one of the release candidate (see "Before the 0.1.0 release" below); the JavaScript and WebAssembly columns were measured earlier and not again. Runs of the whole table varied by about 10% between sessions; within one session of interleaved runs, by 1–3%.

### Before the 0.1.0 release

Five changes: typed code reads the object once, with a `match` on the key filling a slot per field (lookups per field for objects with more than 2N+4 keys); `Validator` walks the sorted schema and the data together when objects are sorted (`serde_json::Map` is a `BTreeMap` unless some crate turns on `preserve_order`); `validate_owned` trims in place with auto-trim and validates owned lists in place; `to_lc` and `to_uc` allocate nothing for unchanged strings; and correctness fixes (`like`, limits on schemas) that do not touch these scenarios.

The old and the new benchmark binaries, and the new one built without fat LTO, run in turn on CPU 9, 3 rounds; the best run of each, and the range of the 3 rounds. Load average 1.4–2.9 (other agents ran on the machine).

| Scenario | before | after | | after, without fat LTO | |
|---|---|---|---|---|---|
| simple, static | 3.72 M/s (3.61–3.72) | 4.01 M/s (3.98–4.01) | +8% | 3.22 M/s (3.18–3.22) | −20% |
| simple, dynamic | 1.88 M/s (1.86–1.88) | 1.97 M/s (1.92–1.97) | +5% | 1.69 M/s | −14% |
| nested, static | 2.38 M/s (2.35–2.38) | 2.48 M/s (2.48) | +4% | 2.09 M/s (2.04–2.09) | −16% |
| nested, dynamic | 1.11 M/s (1.09–1.11) | 1.14 M/s (1.12–1.14) | +3% | 0.98 M/s | −14% |
| invalid, static | 4.46 M/s (4.44–4.46) | 4.71 M/s (4.64–4.71) | +6% | 3.81 M/s (3.79–3.81) | −19% |
| simple, typed | 7.08 M/s (7.07–7.08) | 9.32 M/s (9.03–9.32) | +32% | 8.19 M/s (8.07–8.19) | −12% |
| nested, typed | 4.18 M/s (4.16–4.18) | 4.64 M/s (4.55–4.64) | +11% | 4.24 M/s (4.18–4.24) | −9% |
| simple, static, JSON string | 1.87 M/s (1.86–1.87) | 1.90 M/s (1.86–1.90) | +2% | 1.67 M/s (1.63–1.67) | −12% |
| nested, static, JSON string | 1.44 M/s (1.43–1.44) | 1.50 M/s (1.49–1.50) | +4% | 1.32 M/s (1.29–1.32) | −12% |
| simple, typed, JSON string | 2.71 M/s (2.67–2.71) | 2.97 M/s (2.91–2.97) | +10% | 2.51 M/s (2.47–2.51) | −16% |
| nested, typed, JSON string | 1.83 M/s (1.80–1.83) | 1.88 M/s (1.87–1.88) | +3% | 1.50 M/s (1.48–1.50) | −20% |

The gains of the static `Validator` rows (2–8%) are close to the noise of a session; the typed rows gain the most. "Without fat LTO" is what a crate that depends on livr gets by default: `CARGO_PROFILE_BENCH_LTO=false CARGO_PROFILE_BENCH_CODEGEN_UNITS=16 cargo bench -p livr`. It is 9–20% slower: with `lto = "fat"` and `codegen-units = 1` in the release profile of your application, the numbers of the table above apply.

### Cold start

The first validation in a new process, as on a serverless cold start: the median of 31 processes, pinned to CPU 9.

| | simple, typed | nested, typed |
|---|---|---|
| engine rules (before) | 31–32 µs | 100 µs |
| rules compiled ahead of time | 7 µs | 84 µs |
| a warm validation, for comparison | 0.5 µs | 0.6 µs |

- Rules built on first use (`one_of`, `like`) get their registry only when they compile other rules (`or`), with `CompileCtx::lazy`: building the registry of the built-in rules costs about 25 µs on the first use in a process. With an empty registry instead, the simple form took 15 µs.
- The first regular expression of a process costs about 60 µs (the next ones about 11 µs): most of the cold start of the nested form, which has `like`.
- `Validator::new` with the same schemas: 31 µs and 99 µs for the first validation, about 3 µs and 4 µs later (the schema is compiled on every call there).

### Rules of `schema!` compiled ahead of time

`schema!` used to embed the rules of each field as JSON and compile them with the engine on first use: a `Chain` of `CompiledRule`s, run in a loop with a `match` per rule. It now generates a function per field that calls each rule directly. Rules without heap data are constants (`MaxLength::new(50.0)`); the others are built once from their arguments (`one_of`, `like`). Metarules inside a chain, aliases and custom rules still go through the engine.

The old and the new benchmark binaries, run in turn on CPU 9, 3 rounds (one disturbed run of the old one left out):

| Scenario | engine rules | rules compiled ahead of time | |
|---|---|---|---|
| simple, typed | 5.58–5.59 M/s | 7.07–7.23 M/s | +27% |
| nested, typed | 3.35–3.36 M/s | 4.03–4.08 M/s | +20% |
| simple, typed, JSON string | 2.45–2.46 M/s | 2.66–2.70 M/s | +9% |
| nested, typed, JSON string | 1.65–1.69 M/s | 1.79–1.80 M/s | +7% |

The gain is the dispatch: the compiler sees the whole chain of a field and inlines the rules into it. With data from a JSON string, parsing takes most of the time, so the gain is smaller. The benchmark binary got smaller too (2.21 MB instead of 2.26 MB).

## What the numbers say

- **Typed schemas are the fast path,** 1.9–2.2 times js-validator-livr. Generated code validates straight into the struct: no output map, no keys, rules called directly. Only strings are copied.
- **`Validator::validate` on borrowed data is about as fast as JavaScript.** The output is a new `serde_json::Value`, so every key and every string of the output is copied. Most of the time goes there (measured: about 180 ns of the 270 ns of the simple form, before the release candidate). In JavaScript, strings are shared and objects are cheap. A faster allocator (mimalloc) did not change this.
- **Data from a JSON string:** Rust is 1.2–1.9 times faster. `validate_owned` moves the parsed values to the output instead of copying them, and typed schemas do too. Parsing the JSON is most of the time.
- **Dynamic validators:** about as fast as JavaScript. The names and rules of a schema are compiled into one allocation each, and compiled regular expressions are shared by pattern.
- **WebAssembly is 5–15 times slower than js-validator-livr.** Every call pays for the boundary:
  - a string round trip with nothing done in it costs 313 ns, more than a whole validation in JavaScript;
  - `JSON.stringify` and `JSON.parse` cost 84 ns and 273 ns;
  - reading objects property by property (`serde-wasm-bindgen`) is slower still;
  - inside WebAssembly, the validation itself is about 1.5 times slower than native code (381 ns for the simple form).

  In JavaScript, use js-validator-livr. The WebAssembly build is 914 KB, mostly the `regex` crate.
