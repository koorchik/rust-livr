//! LIVR for JavaScript: the validator of livr-core compiled to WebAssembly.
//!
//! ```js
//! const { Validator } = require('livr-wasm');
//! const validator = new Validator({ name: 'required', age: 'positive_integer' });
//! validator.validate({ name: 'Jane', age: '42' }); // { output: { name: 'Jane', age: 42 } }
//! validator.validate({ age: 'x' });                // { errors: { name: 'REQUIRED', age: 'NOT_POSITIVE_INTEGER' } }
//! validator.validateJson('{"name": "Jane"}');      // the same, for data as a JSON string
//! ```

use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// A compiled LIVR schema.
#[wasm_bindgen]
pub struct Validator {
    inner: livr_core::Validator,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<livr_core::Errors>,
}

// Objects as plain JavaScript objects, not maps
const SERIALIZER: serde_wasm_bindgen::Serializer =
    serde_wasm_bindgen::Serializer::json_compatible();

fn to_js(result: Result<Value, livr_core::Errors>) -> Result<JsValue, JsError> {
    let result = match result {
        Ok(output) => ValidationResult {
            output: Some(output),
            errors: None,
        },
        Err(errors) => ValidationResult {
            output: None,
            errors: Some(errors),
        },
    };
    result
        .serialize(&SERIALIZER)
        .map_err(|error| JsError::new(&error.to_string()))
}

#[wasm_bindgen]
impl Validator {
    /// Compiles the schema; `autoTrim` trims all strings of the data.
    ///
    /// # Errors
    ///
    /// An error for an invalid schema.
    #[wasm_bindgen(constructor)]
    pub fn new(schema: JsValue, auto_trim: Option<bool>) -> Result<Validator, JsError> {
        let schema: Value = serde_wasm_bindgen::from_value(schema)
            .map_err(|error| JsError::new(&error.to_string()))?;
        let inner = livr_core::Validator::builder(&schema)
            .auto_trim(auto_trim.unwrap_or(false))
            .build()
            .map_err(|error| JsError::new(&error.to_string()))?;
        Ok(Self { inner })
    }

    /// Validates data: `{ output }` or `{ errors }`.
    ///
    /// # Errors
    ///
    /// An error when the data cannot be read.
    pub fn validate(&self, data: JsValue) -> Result<JsValue, JsError> {
        let data: Value = serde_wasm_bindgen::from_value(data)
            .map_err(|error| JsError::new(&error.to_string()))?;
        to_js(self.inner.validate_owned(data))
    }

    /// Compiles a schema given as a JSON string.
    ///
    /// # Errors
    ///
    /// An error for an invalid schema.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(schema: &str, auto_trim: Option<bool>) -> Result<Validator, JsError> {
        let schema: Value =
            serde_json::from_str(schema).map_err(|error| JsError::new(&error.to_string()))?;
        let inner = livr_core::Validator::builder(&schema)
            .auto_trim(auto_trim.unwrap_or(false))
            .build()
            .map_err(|error| JsError::new(&error.to_string()))?;
        Ok(Self { inner })
    }

    /// Validates data in a JSON string, with the result as a JSON string:
    /// `{"output": ...}` or `{"errors": ...}`. Data crosses between
    /// JavaScript and WebAssembly as one string, the fastest way.
    ///
    /// # Errors
    ///
    /// An error for invalid JSON.
    #[wasm_bindgen(js_name = validateJsonToJson)]
    pub fn validate_json_to_json(&self, json: &str) -> Result<String, JsError> {
        let data: Value =
            serde_json::from_str(json).map_err(|error| JsError::new(&error.to_string()))?;
        let result = match self.inner.validate_owned(data) {
            Ok(output) => ValidationResult {
                output: Some(output),
                errors: None,
            },
            Err(errors) => ValidationResult {
                output: None,
                errors: Some(errors),
            },
        };
        serde_json::to_string(&result).map_err(|error| JsError::new(&error.to_string()))
    }

    /// Validates data in a JSON string: `{ output }` or `{ errors }`.
    ///
    /// # Errors
    ///
    /// An error for invalid JSON.
    #[wasm_bindgen(js_name = validateJson)]
    pub fn validate_json(&self, json: &str) -> Result<JsValue, JsError> {
        let data: Value =
            serde_json::from_str(json).map_err(|error| JsError::new(&error.to_string()))?;
        to_js(self.inner.validate_owned(data))
    }
}
