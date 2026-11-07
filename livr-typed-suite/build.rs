//! Generates a `livr::schema!` and a test for every case of the LIVR test
//! suites (livr-core/tests/test_suite and test_suite_2.5). Cases that fix the
//! current time (`options.json`) get a `#[clock(now)]` with that time.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde_json::Value;

fn main() {
    let mut out = String::new();
    for (root, prefix) in [("test_suite", ""), ("test_suite_2.5", "v2_5_")] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../livr-core/tests")
            .join(root);
        println!("cargo::rerun-if-changed={}", root.display());
        if root.is_dir() {
            cases(&root, prefix, &mut out);
        }
    }
    fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("cases.rs"),
        out,
    )
    .unwrap();
}

fn cases(root: &Path, prefix: &str, out: &mut String) {
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
            let module = format!("{prefix}{suite}_{}", case.replace('-', "_"));
            let read = |file: &str| {
                fs::read_to_string(dir.join(file))
                    .ok()
                    .map(|text| serde_json::from_str::<Value>(&text).unwrap())
            };
            let rules = read("rules.json").unwrap();
            let input = read("input.json").unwrap();
            let mut attrs = String::new();
            let mut clock = String::new();
            if let Some(now) = read("options.json").and_then(|options| options.get("now").cloned())
            {
                let seconds = unix_seconds(now.as_str().expect("now is a string"));
                attrs.push_str("        #[clock(now)]\n");
                clock = format!(
                    "\n    fn now() -> std::time::SystemTime {{\n        std::time::UNIX_EPOCH + std::time::Duration::from_secs({seconds})\n    }}\n"
                );
            }
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
mod {module} {{{clock}
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
}

// `YYYY-MM-DDTHH:MM:SSZ`, the form of the `now` option of the suite, as
// seconds since 1970-01-01T00:00:00Z ("days_from_civil" by Howard Hinnant)
fn unix_seconds(now: &str) -> u64 {
    let number = |range: std::ops::Range<usize>| -> i64 { now[range].parse().expect("a number") };
    assert!(now.len() == 20 && now.ends_with('Z'), "now: {now}");
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds = days * 86_400 + number(11..13) * 3600 + number(14..16) * 60 + number(17..19);
    u64::try_from(seconds).expect("after 1970")
}
