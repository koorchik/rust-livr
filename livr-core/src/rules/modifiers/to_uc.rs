use crate::rules::prelude::*;

/// `to_uc`: converts the value to upper case.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToUc;

impl ToUc {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// None, the signature is the one of all rule builders.
    pub fn build(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) || !is_primitive(value) {
            return Outcome::Valid;
        }
        let str = to_js_string(value);
        // Most strings are already in upper case: no new string for them
        if value.is_string() && is_upper_case(&str) {
            return Outcome::Valid;
        }
        Outcome::Value(Value::String(str.to_uppercase()))
    }
}

// Whether the string is its own upper case form: no character changes. A
// string without changing characters is unchanged by `str::to_uppercase`
fn is_upper_case(str: &str) -> bool {
    if str.is_ascii() {
        return !str.bytes().any(|byte| byte.is_ascii_lowercase());
    }
    str.chars().all(|char| {
        let mut converted = char.to_uppercase();
        converted.next() == Some(char) && converted.next().is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::is_upper_case;

    #[test]
    fn finds_strings_that_do_not_change() {
        for str in [
            "", "abc", "ABC", "aBc", "київ", "КИЇВ", "Σ", "σς", "ǅ", "ß", "ﬁ", "İ", "a\u{301}",
            "日本", "1-2",
        ] {
            assert_eq!(is_upper_case(str), str.to_uppercase() == str, "{str:?}");
        }
    }
}
