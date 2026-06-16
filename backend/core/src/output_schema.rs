use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An output schema that agents can declare for structured responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchema {
    /// JSON Schema definition for the expected output
    pub schema: Value,
    /// Whether to enforce strict validation (reject on missing fields)
    pub strict: bool,
    /// Maximum number of retry attempts on validation failure
    pub max_retries: u32,
    /// Human-readable description of expected output
    pub description: Option<String>,
}

impl OutputSchema {
    /// Create a new output schema from a JSON Schema value
    pub fn new(schema: Value) -> Self {
        Self {
            schema,
            strict: false,
            max_retries: 3,
            description: None,
        }
    }

    /// Validate a JSON value against this schema
    pub fn validate(&self, value: &Value) -> Result<(), ValidationError> {
        let mut errors = Vec::new();
        validate_value(&self.schema, value, "", &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError { errors })
        }
    }

    /// Try to parse a string as JSON and validate
    pub fn validate_str(&self, output: &str) -> Result<Value, ValidationError> {
        let value: Value = serde_json::from_str(output).map_err(|e| ValidationError {
            errors: vec![ValidationErrorItem {
                path: "$".into(),
                message: format!("Invalid JSON: {}", e),
                kind: ValidationErrorKind::InvalidValue,
            }],
        })?;
        self.validate(&value)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub errors: Vec<ValidationErrorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorItem {
    pub path: String,
    pub message: String,
    pub kind: ValidationErrorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationErrorKind {
    MissingField,
    WrongType,
    InvalidValue,
    ExtraField,
    Custom,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for err in &self.errors {
            writeln!(f, "{}: {} ({:?})", err.path, err.message, err.kind)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationError {}

/// Validate a JSON value against a JSON Schema
fn validate_value(schema: &Value, value: &Value, path: &str, errors: &mut Vec<ValidationErrorItem>) {
    let schema = match schema.as_object() {
        Some(s) => s,
        None => return,
    };

    // Check type
    if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
        let actual_type = match value {
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Null => "null",
        };
        if expected_type != actual_type {
            errors.push(ValidationErrorItem {
                path: path.into(),
                message: format!("Expected type '{}', got '{}'", expected_type, actual_type),
                kind: ValidationErrorKind::WrongType,
            });
            return;
        }
    }

    // Check required fields (for objects)
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        if let Some(obj) = value.as_object() {
            for field in required {
                if let Some(field_name) = field.as_str() {
                    if !obj.contains_key(field_name) {
                        errors.push(ValidationErrorItem {
                            path: format!("{}.{}", path, field_name),
                            message: format!("Required field '{}' is missing", field_name),
                            kind: ValidationErrorKind::MissingField,
                        });
                    }
                }
            }
        }
    }

    // Check properties (for objects)
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        if let Some(obj) = value.as_object() {
            for (key, prop_schema) in properties {
                let field_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                if let Some(field_value) = obj.get(key) {
                    validate_value(prop_schema, field_value, &field_path, errors);
                }
            }
        }
    }

    // Check enum values
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.contains(value) {
            errors.push(ValidationErrorItem {
                path: path.into(),
                message: format!("Value {:?} not in enum {:?}", value, enum_values),
                kind: ValidationErrorKind::InvalidValue,
            });
        }
    }

    // Check string constraints
    if let Some(s) = value.as_str() {
        if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if (s.len() as u64) < min_len {
                errors.push(ValidationErrorItem {
                    path: path.into(),
                    message: format!("String length {} < minLength {}", s.len(), min_len),
                    kind: ValidationErrorKind::InvalidValue,
                });
            }
        }
        if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            if (s.len() as u64) > max_len {
                errors.push(ValidationErrorItem {
                    path: path.into(),
                    message: format!("String length {} > maxLength {}", s.len(), max_len),
                    kind: ValidationErrorKind::InvalidValue,
                });
            }
        }
    }

    // Check number constraints
    if let Some(n) = value.as_f64() {
        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            if n < min {
                errors.push(ValidationErrorItem {
                    path: path.into(),
                    message: format!("Value {} < minimum {}", n, min),
                    kind: ValidationErrorKind::InvalidValue,
                });
            }
        }
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            if n > max {
                errors.push(ValidationErrorItem {
                    path: path.into(),
                    message: format!("Value {} > maximum {}", n, max),
                    kind: ValidationErrorKind::InvalidValue,
                });
            }
        }
    }

    // Check additionalProperties in strict mode
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        if let Some(obj) = value.as_object() {
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for key in obj.keys() {
                    if !properties.contains_key(key) {
                        errors.push(ValidationErrorItem {
                            path: format!("{}.{}", path, key),
                            message: format!("Additional property '{}' not allowed", key),
                            kind: ValidationErrorKind::ExtraField,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_object() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        }));
        let value = serde_json::json!({"name": "Alice", "age": 30});
        assert!(schema.validate(&value).is_ok());
    }

    #[test]
    fn test_validate_missing_required_field() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        }));
        let value = serde_json::json!({"name": "Alice"});
        let err = schema.validate(&value).unwrap_err();
        assert_eq!(err.errors.len(), 1);
        assert_eq!(err.errors[0].kind, ValidationErrorKind::MissingField);
    }

    #[test]
    fn test_validate_wrong_type() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        }));
        let value = serde_json::json!({"name": 42});
        let err = schema.validate(&value).unwrap_err();
        assert_eq!(err.errors[0].kind, ValidationErrorKind::WrongType);
    }

    #[test]
    fn test_validate_str_valid_json() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object",
            "required": ["result"],
            "properties": {
                "result": { "type": "string" }
            }
        }));
        assert!(schema.validate_str(r#"{"result": "hello"}"#).is_ok());
    }

    #[test]
    fn test_validate_str_invalid_json() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object"
        }));
        // Plain text is not valid JSON
        let result = schema.validate_str("hello world");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_number_range() {
        let schema = OutputSchema::new(serde_json::json!({
            "type": "object",
            "required": ["score"],
            "properties": {
                "score": { "type": "number", "minimum": 0, "maximum": 100 }
            }
        }));
        assert!(schema.validate(&serde_json::json!({"score": 50})).is_ok());
        let err = schema.validate(&serde_json::json!({"score": 150})).unwrap_err();
        assert!(err.errors[0].message.contains("maximum"));
    }

    #[test]
    fn test_validate_strict_additional_properties() {
        let schema = OutputSchema {
            schema: serde_json::json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "additionalProperties": false
            }),
            strict: true,
            max_retries: 3,
            description: None,
        };
        assert!(schema.validate(&serde_json::json!({"name": "Alice"})).is_ok());
        let err = schema.validate(&serde_json::json!({"name": "Alice", "extra": true})).unwrap_err();
        assert_eq!(err.errors[0].kind, ValidationErrorKind::ExtraField);
    }
}
