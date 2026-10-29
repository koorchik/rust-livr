//! Schema errors are compile errors.

#[test]
fn schema_errors_are_compile_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
