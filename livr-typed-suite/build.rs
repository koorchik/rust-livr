//! Generates a `livr::schema!` and a test for every case of the LIVR test
//! suite (livr-core/tests/test_suite).

use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde_json::Value;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../livr-core/tests/test_suite");
    println!("cargo::rerun-if-changed={}", root.display());
    let mut out = String::new();
    for suite in [
        "positive",
        "negative",
        "aliases_positive",
        "aliases_negative",
    ] {
        let mut dirs: Vec<_> = fs::read_dir(root.join(suite))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            let case = dir.file_name().unwrap().to_string_lossy().into_owned();
            let module = format!("{suite}_{}", case.replace('-', "_"));
            let read = |file: &str| {
                fs::read_to_string(dir.join(file))
                    .ok()
                    .map(|text| serde_json::from_str::<Value>(&text).unwrap())
            };
            let rules = read("rules.json").unwrap();
            let input = read("input.json").unwrap();
            let mut attrs = String::new();
            if let Some(Value::Array(aliases)) = read("aliases.json") {
                for alias in aliases {
                    let name = alias["name"].as_str().unwrap();
                    let error = match alias.get("error") {
                        Some(Value::String(error)) => format!(", error = {error:?}"),
                        _ => String::new(),
                    };
                    writeln!(
                        attrs,
                        "        #[alias({name} = {}{error})]",
                        alias["rules"]
                    )
                    .unwrap();
                }
            }
            let fields: String = rules
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, rules)| format!("            {key:?}: {rules},\n"))
                .collect();
            let (expected, check) = match read("output.json") {
                Some(output) => (output, "check_output"),
                None => (read("errors.json").unwrap(), "check_errors"),
            };
            write!(
                out,
                r#"
mod {module} {{
    livr::schema! {{
        #[derive(Debug)]
{attrs}        pub struct Schema {{
{fields}        }}
    }}

    #[test]
    fn run() {{
        let input: serde_json::Value = serde_json::from_str({input:?}).unwrap();
        let expected: serde_json::Value = serde_json::from_str({expected:?}).unwrap();
        crate::{check}(Schema::validate(&input), &expected);
        crate::{check}(Schema::validate_owned(input), &expected);
    }}
}}
"#,
                input = input.to_string(),
                expected = expected.to_string(),
            )
            .unwrap();
        }
    }
    fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("cases.rs"),
        out,
    )
    .unwrap();
}
