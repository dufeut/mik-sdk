//! OpenAPI schema generation using utoipa.
//!
//! This module provides type-safe OpenAPI schema builders using utoipa,
//! replacing raw string concatenation. All code runs at compile time only,
//! with zero runtime cost in the final WASM binary.
//!
//! Some helpers are provided for future use (e.g., `enum_schema`, `object_schema`).

#![allow(dead_code)] // Helpers provided for future schema building

use utoipa::openapi::{
    ArrayBuilder, ObjectBuilder, RefOr, Schema,
    schema::{SchemaFormat, SchemaType},
};

// ============================================================================
// BASIC TYPE MAPPING
// ============================================================================

/// Map a Rust type name to an OpenAPI schema.
pub fn rust_type_to_schema(type_name: &str) -> RefOr<Schema> {
    match type_name {
        "String" | "str" => RefOr::T(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .build()
                .into(),
        ),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            RefOr::T(
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(utoipa::openapi::Type::Integer))
                    .build()
                    .into(),
            )
        },
        "f32" | "f64" => RefOr::T(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Number))
                .build()
                .into(),
        ),
        "bool" => RefOr::T(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Boolean))
                .build()
                .into(),
        ),
        // Custom type - reference to schema
        custom => RefOr::Ref(utoipa::openapi::Ref::from_schema_name(custom)),
    }
}

/// Make a schema nullable (for `Option<T>`).
/// Returns JSON string with nullable:true added.
pub fn make_nullable_json(schema_json: &str) -> String {
    // Insert nullable:true after the opening brace
    if schema_json.starts_with('{') && schema_json.len() > 1 {
        format!("{{\"nullable\":true,{}", &schema_json[1..])
    } else {
        schema_json.to_string()
    }
}

/// Build an array schema (for `Vec<T>`).
pub fn array_schema(items: RefOr<Schema>) -> Schema {
    ArrayBuilder::new().items(items).build().into()
}

// ============================================================================
// FIELD CONSTRAINTS
// ============================================================================

use crate::derive::XAttrValue;

/// Field constraints from `#[field(...)]` attributes.
#[derive(Default)]
pub struct FieldConstraints {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub format: Option<String>,
    pub pattern: Option<String>,
    pub description: Option<String>,
    /// OpenAPI x-* extension attributes
    pub x_attrs: Vec<(String, XAttrValue)>,
}

/// Apply field constraints to an `ObjectBuilder`.
pub fn apply_constraints(
    mut builder: ObjectBuilder,
    constraints: &FieldConstraints,
    is_string: bool,
) -> ObjectBuilder {
    if let Some(ref desc) = constraints.description {
        builder = builder.description(Some(desc.clone()));
    }
    if let Some(ref fmt) = constraints.format {
        builder = builder.format(Some(SchemaFormat::Custom(fmt.clone())));
    }
    if let Some(ref pattern) = constraints.pattern {
        builder = builder.pattern(Some(pattern.clone()));
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    if let Some(min) = constraints.min {
        if is_string {
            builder = builder.min_length(Some(min as usize));
        } else {
            builder = builder.minimum(Some(min as f64));
        }
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    if let Some(max) = constraints.max {
        if is_string {
            builder = builder.max_length(Some(max as usize));
        } else {
            builder = builder.maximum(Some(max as f64));
        }
    }
    builder
}

// ============================================================================
// ENUM SCHEMA
// ============================================================================

/// Build an enum schema with string variants.
pub fn enum_schema(variants: &[&str]) -> Schema {
    ObjectBuilder::new()
        .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
        .enum_values(Some(variants.iter().map(|&s| s.to_string())))
        .build()
        .into()
}

// ============================================================================
// OBJECT SCHEMA
// ============================================================================

/// A field definition for building object schemas.
pub struct FieldDef {
    pub name: String,
    pub schema: RefOr<Schema>,
    pub required: bool,
}

/// Build an object schema from field definitions.
pub fn object_schema(fields: Vec<FieldDef>) -> Schema {
    let mut builder = ObjectBuilder::new();

    for field in &fields {
        builder = builder.property(&field.name, field.schema.clone());
        if field.required {
            builder = builder.required(&field.name);
        }
    }

    builder.build().into()
}

/// A field definition using raw JSON strings (preserves nullable).
pub struct JsonFieldDef {
    pub name: String,
    pub schema_json: String,
    pub required: bool,
    /// OpenAPI x-* extension attributes for this field
    pub x_attrs: Vec<(String, XAttrValue)>,
}

/// Format x-attrs as JSON key-value pairs.
/// Returns empty string if no x-attrs, or ",\"x-foo\":value,..." format.
fn format_x_attrs(x_attrs: &[(String, XAttrValue)]) -> String {
    if x_attrs.is_empty() {
        return String::new();
    }

    let parts: Vec<String> = x_attrs
        .iter()
        .map(|(name, value)| {
            let json_value = match value {
                XAttrValue::String(s) => format!("\"{}\"", escape_json_string(s)),
                XAttrValue::Bool(b) => b.to_string(),
                XAttrValue::Int(n) => n.to_string(),
                XAttrValue::Float(f) => f.to_string(),
            };
            format!("\"{name}\":{json_value}")
        })
        .collect();

    format!(",{}", parts.join(","))
}

/// Escape a string for use in JSON output.
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c => result.push(c),
        }
    }
    result
}

/// Build an object schema as JSON string from field definitions.
/// This preserves nullable and other properties that utoipa might drop.
pub fn object_schema_json(fields: Vec<JsonFieldDef>) -> String {
    let mut properties = Vec::new();
    let mut required_fields = Vec::new();

    for field in fields {
        // Append x-attrs to the field schema if present
        let schema_with_x_attrs = if field.x_attrs.is_empty() {
            field.schema_json
        } else {
            // Insert x-attrs before the closing brace of the schema
            let x_attrs_json = format_x_attrs(&field.x_attrs);
            if field.schema_json.ends_with('}') {
                format!(
                    "{}{}}}",
                    &field.schema_json[..field.schema_json.len() - 1],
                    x_attrs_json
                )
            } else {
                field.schema_json
            }
        };
        properties.push(format!(r#""{}":{}"#, field.name, schema_with_x_attrs));
        if field.required {
            required_fields.push(format!(r#""{}""#, field.name));
        }
    }

    if required_fields.is_empty() {
        format!(
            r#"{{"type":"object","properties":{{{}}}}}"#,
            properties.join(",")
        )
    } else {
        format!(
            r#"{{"type":"object","required":[{}],"properties":{{{}}}}}"#,
            required_fields.join(","),
            properties.join(",")
        )
    }
}

// ============================================================================
// RFC 7807 PROBLEM DETAILS
// ============================================================================

/// Build the RFC 7807 Problem Details schema.
///
/// This is a reusable constant schema for error responses that complies
/// with [RFC 7807](https://datatracker.ietf.org/doc/html/rfc7807).
pub fn problem_details_schema() -> Schema {
    ObjectBuilder::new()
        .description(Some("RFC 7807 Problem Details for HTTP APIs"))
        .property(
            "type",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .description(Some("URI reference identifying the problem type"))
                .default(Some(serde_json::Value::String("about:blank".to_string())))
                .build(),
        )
        .required("type")
        .property(
            "title",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .description(Some("Short human-readable summary of the problem"))
                .build(),
        )
        .required("title")
        .property(
            "status",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Integer))
                .description(Some("HTTP status code"))
                .minimum(Some(100.0))
                .maximum(Some(599.0))
                .build(),
        )
        .required("status")
        .property(
            "detail",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .description(Some(
                    "Human-readable explanation specific to this occurrence",
                ))
                .build(),
        )
        .property(
            "instance",
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .description(Some("URI reference identifying the specific occurrence"))
                .build(),
        )
        .build()
        .into()
}

/// Get the RFC 7807 Problem Details schema as JSON string.
pub fn problem_details_json() -> String {
    schema_to_json(&problem_details_schema())
}

// ============================================================================
// SERIALIZATION
// ============================================================================

/// Serialize a schema to JSON string.
pub fn schema_to_json(schema: &Schema) -> String {
    serde_json::to_string(schema).unwrap_or_else(|_| r#"{"type":"object"}"#.to_string())
}

/// Serialize a `RefOr<Schema>` to JSON string.
pub fn ref_or_schema_to_json(schema: &RefOr<Schema>) -> String {
    serde_json::to_string(schema).unwrap_or_else(|_| r#"{"type":"object"}"#.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_schema() {
        let schema = rust_type_to_schema("String");
        let json = ref_or_schema_to_json(&schema);
        assert!(json.contains("\"type\":\"string\""));
    }

    #[test]
    fn test_integer_schema() {
        let schema = rust_type_to_schema("i32");
        let json = ref_or_schema_to_json(&schema);
        assert!(json.contains("\"type\":\"integer\""));
    }

    #[test]
    fn test_enum_schema() {
        let schema = enum_schema(&["active", "inactive"]);
        let json = schema_to_json(&schema);
        assert!(json.contains("\"type\":\"string\""));
        assert!(json.contains("\"enum\""));
        assert!(json.contains("\"active\""));
        assert!(json.contains("\"inactive\""));
    }

    #[test]
    fn test_nullable_schema() {
        let schema = rust_type_to_schema("String");
        let json = ref_or_schema_to_json(&schema);
        let nullable_json = make_nullable_json(&json);
        assert!(nullable_json.contains("\"nullable\":true"));
    }

    #[test]
    fn test_array_schema() {
        let items = rust_type_to_schema("i32");
        let arr = array_schema(items);
        let json = schema_to_json(&arr);
        assert!(json.contains("\"type\":\"array\""));
        assert!(json.contains("\"items\""));
    }

    #[test]
    fn test_custom_type_ref() {
        let schema = rust_type_to_schema("MyCustomType");
        let json = ref_or_schema_to_json(&schema);
        assert!(json.contains("\"$ref\""));
        assert!(json.contains("MyCustomType"));
    }

    #[test]
    fn test_problem_details_schema() {
        let json = super::problem_details_json();
        // Check required fields
        assert!(
            json.contains("\"type\":\"object\""),
            "Should be object type"
        );
        assert!(json.contains("\"required\""), "Should have required fields");
        // Check all RFC 7807 properties
        assert!(json.contains("\"type\""), "Should have type property");
        assert!(json.contains("\"title\""), "Should have title property");
        assert!(json.contains("\"status\""), "Should have status property");
        assert!(json.contains("\"detail\""), "Should have detail property");
        assert!(
            json.contains("\"instance\""),
            "Should have instance property"
        );
        // Check descriptions
        assert!(
            json.contains("RFC 7807"),
            "Should have RFC 7807 description"
        );
    }
}
