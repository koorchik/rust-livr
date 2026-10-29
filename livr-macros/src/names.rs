//! Rust names for the keys of schemas.

use proc_macro2::Span;
use syn::Ident;

/// `first_name`, `first-name`, `firstName` become `FirstName`. Only
/// characters that can be in an identifier are kept (`²` is not one).
pub fn pascal_case(key: &str) -> String {
    let mut name = String::with_capacity(key.len());
    let mut is_word_start = true;
    let mut previous_lower = false;
    for char in key.chars() {
        if !char.is_alphanumeric() || !unicode_ident::is_xid_continue(char) {
            is_word_start = true;
            previous_lower = false;
            continue;
        }
        if is_word_start || (char.is_uppercase() && previous_lower) {
            name.extend(char.to_uppercase());
        } else {
            name.push(char);
        }
        is_word_start = false;
        previous_lower = char.is_lowercase() || char.is_ascii_digit();
    }
    name
}

/// The name of a variant for a value: `"admin"` becomes `Admin`, `"1"`
/// becomes `V1`, `""` becomes `Empty`, `"self"` becomes `Self_` (`Self` is a
/// keyword), and a value without letters or digits, like `"²"`, becomes `V`.
pub fn variant_name(value: &str) -> String {
    let name = pascal_case(value);
    // Upper case can give characters that are not in identifiers
    let name: String = name
        .chars()
        .filter(|char| unicode_ident::is_xid_continue(*char))
        .collect();
    match name.chars().next() {
        None if value.is_empty() => "Empty".to_owned(),
        None => "V".to_owned(),
        // `V` starts an identifier, the other characters continue it
        Some(first) if !first.is_ascii_alphabetic() => format!("V{name}"),
        Some(_) if name == "Self" => "Self_".to_owned(),
        Some(_) => ascii_only(&name),
    }
}

/// `firstName`, `first-name` become `first_name`; keywords become raw
/// identifiers. The second value tells whether it differs from the key.
pub fn field_ident(key: &str) -> (Ident, bool) {
    let mut name = String::with_capacity(key.len() + 4);
    let mut previous_lower = false;
    for char in key.chars() {
        if char.is_ascii_alphanumeric() || char == '_' {
            if char.is_ascii_uppercase() {
                if previous_lower {
                    name.push('_');
                }
                name.push(char.to_ascii_lowercase());
            } else {
                name.push(char);
            }
            previous_lower = char.is_ascii_lowercase() || char.is_ascii_digit();
        } else {
            if !name.ends_with('_') {
                name.push('_');
            }
            previous_lower = false;
        }
    }
    if name.is_empty() || name.starts_with(|char: char| char.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if name == "_" {
        name = "_field".to_owned();
    }
    let renamed = name != key;
    let ident = if is_keyword(&name) {
        if matches!(name.as_str(), "self" | "Self" | "super" | "crate") {
            Ident::new(&format!("{name}_"), Span::call_site())
        } else {
            Ident::new_raw(&name, Span::call_site())
        }
    } else {
        Ident::new(&name, Span::call_site())
    };
    (ident, renamed)
}

fn ascii_only(name: &str) -> String {
    let ascii: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
    if ascii.is_empty() {
        "Value".to_owned()
    } else {
        ascii
    }
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "gen"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_names() {
        assert_eq!(pascal_case("first_name"), "FirstName");
        assert_eq!(pascal_case("firstName"), "FirstName");
        assert_eq!(pascal_case("first-name"), "FirstName");
        assert_eq!(variant_name("admin"), "Admin");
        assert_eq!(variant_name("1"), "V1");
        assert_eq!(variant_name(""), "Empty");
        assert_eq!(variant_name("self"), "Self_");
        assert_eq!(variant_name("self_"), "Self_");
        assert_eq!(variant_name("²"), "V");
        assert_eq!(variant_name("a²"), "A");
        assert_eq!(variant_name("київ"), "VКиїв");
        assert_eq!(pascal_case("a²"), "A");
        assert_eq!(field_ident("firstName").0.to_string(), "first_name");
        assert_eq!(field_ident("first-name").0.to_string(), "first_name");
        assert_eq!(field_ident("type").0.to_string(), "r#type");
        assert_eq!(field_ident("__proto__").0.to_string(), "__proto__");
        assert_eq!(field_ident("1a").0.to_string(), "_1a");
        assert!(!field_ident("name").1);
    }
}
