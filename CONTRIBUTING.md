# Contributing

Thank you for helping. Bug reports, fixes, new tests and documentation are all welcome.

## Reporting a bug

Open an issue with:
- the schema and the input data;
- what you got and what you expected;
- the versions of `livr` and Rust.

If the behavior differs from another LIVR implementation, say which one. The rules are defined by the [LIVR specification](https://livr-spec.org); some differences are intentional and listed in the README, under "Semantics".

Security issues: see [SECURITY.md](SECURITY.md), not public issues.

## Making a change

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
```

- **Tests:** every fix comes with a test. The LIVR test suite in `livr-core/tests/test_suite` is a copy of the one in the specification; change it there first.
- **Rules:** one rule per file, as described in [CLAUDE.md](CLAUDE.md), which documents the architecture and conventions.
- **Performance:** changes to hot paths need benchmark numbers (`cargo bench -p livr`, see [BENCHMARKS.md](BENCHMARKS.md)).
- **MSRV:** the minimum supported Rust is in `Cargo.toml` (`rust-version`); CI checks it.
- **`schema!`:** type inference must stay sound. For every valid input, the output fits the generated types.

## License

By contributing, you agree that your contributions are licensed under the [MIT license](LICENSE).
