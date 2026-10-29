//! Error codes and the errors of validation and of schemas.

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// An error code, like `REQUIRED`. Cheap to clone: the codes of the built-in
/// rules are static strings, the custom error of an alias is shared.
#[derive(Clone)]
pub struct Code(CodeRepr);

#[derive(Clone)]
enum CodeRepr {
    Static(&'static str),
    Shared(Arc<str>),
}

impl Code {
    /// A code from a static string, without allocation.
    #[must_use]
    pub const fn from_static(code: &'static str) -> Self {
        Self(CodeRepr::Static(code))
    }

    /// The code as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            CodeRepr::Static(code) => code,
            CodeRepr::Shared(code) => code,
        }
    }
}

impl Deref for Code {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for Code {
    fn from(code: &'static str) -> Self {
        Self::from_static(code)
    }
}

impl From<String> for Code {
    fn from(code: String) -> Self {
        Self(CodeRepr::Shared(code.into()))
    }
}

impl From<Arc<str>> for Code {
    fn from(code: Arc<str>) -> Self {
        Self(CodeRepr::Shared(code))
    }
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Code {}

// Like `str`, as equality compares the strings
impl Hash for Code {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq<str> for Code {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Code {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// The errors of a value, in the shape of the value: a code for a field, the
/// errors of the fields for an object, the errors of the items for a list
/// (`None` for valid items).
///
/// The enum is exhaustive on purpose: it mirrors the JSON of LIVR errors (a
/// string, an object or a list), which the specification fixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorTree {
    /// The error code of a value, like `"REQUIRED"`.
    Code(Code),
    /// The errors of the invalid fields of an object, by field name.
    Object(BTreeMap<String, ErrorTree>),
    /// The errors of the items of a list, `None` for the valid ones.
    List(Vec<Option<ErrorTree>>),
}

impl ErrorTree {
    /// The error with a static code.
    #[must_use]
    pub const fn code(code: &'static str) -> Self {
        Self::Code(Code::from_static(code))
    }

    /// The code, when the error is a code.
    #[must_use]
    pub fn as_code(&self) -> Option<&str> {
        match self {
            Self::Code(code) => Some(code),
            _ => None,
        }
    }
}

impl Serialize for ErrorTree {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Code(code) => code.serialize(serializer),
            Self::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (name, error) in fields {
                    map.serialize_entry(name, error)?;
                }
                map.end()
            }
            Self::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for error in items {
                    seq.serialize_element(error)?;
                }
                seq.end()
            }
        }
    }
}

impl fmt::Display for ErrorTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_json(self, f)
    }
}

/// The errors of [`Validator::validate`](crate::Validator::validate).
///
/// The enum is exhaustive on purpose: it mirrors the JSON of LIVR errors for
/// a whole object, `"FORMAT_ERROR"` or the errors of the fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Errors {
    /// The data is not an object. Serializes to `"FORMAT_ERROR"`.
    Format,
    /// The errors of the invalid fields, by field name.
    Fields(BTreeMap<String, ErrorTree>),
}

impl Errors {
    /// The error of a field.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&ErrorTree> {
        match self {
            Self::Format => None,
            Self::Fields(fields) => fields.get(name),
        }
    }

    /// The errors as a tree: `FORMAT_ERROR`, or the errors of the fields.
    #[must_use]
    pub fn into_tree(self) -> ErrorTree {
        match self {
            Self::Format => ErrorTree::code("FORMAT_ERROR"),
            Self::Fields(fields) => ErrorTree::Object(fields),
        }
    }
}

impl Serialize for Errors {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Format => serializer.serialize_str("FORMAT_ERROR"),
            Self::Fields(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (name, error) in fields {
                    map.serialize_entry(name, error)?;
                }
                map.end()
            }
        }
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_json(self, f)
    }
}

impl std::error::Error for Errors {}

fn write_json(value: &impl Serialize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let json = serde_json::to_string(value).map_err(|_| fmt::Error)?;
    f.write_str(&json)
}

/// An invalid schema: an unknown rule, invalid arguments of a rule, a
/// recursive alias. Returned when a validator is built, never by `validate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    message: String,
}

impl SchemaError {
    /// An error with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The message of the error.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LIVR schema error: {}", self.message)
    }
}

impl std::error::Error for SchemaError {}

/// The error of [`Validator::validate_into`](crate::Validator::validate_into).
///
/// Its message says which step failed; the errors of that step are its
/// [`source`](std::error::Error::source).
#[derive(Debug)]
#[non_exhaustive]
pub enum ValidateIntoError {
    /// The data is invalid.
    Invalid(Errors),
    /// The output does not fit the type.
    Convert(serde_json::Error),
}

impl fmt::Display for ValidateIntoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The inner error is the source, not repeated here: error reporters
        // print the whole chain
        match self {
            Self::Invalid(_) => f.write_str("invalid data"),
            Self::Convert(_) => f.write_str("the output does not fit the type"),
        }
    }
}

impl std::error::Error for ValidateIntoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invalid(errors) => Some(errors),
            Self::Convert(error) => Some(error),
        }
    }
}
