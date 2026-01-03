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

/// Field constraints from `#[field(...)]` attributes.
#[derive(Default)]
pub struct FieldConstraints {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub format: Option<String>,
    pub pattern: Option<String>,
    pub description: Option<String>,
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
}

/// Build an object schema as JSON string from field definitions.
/// This preserves nullable and other properties that utoipa might drop.
pub fn object_schema_json(fields: Vec<JsonFieldDef>) -> String {
    let mut properties = Vec::new();
    let mut required_fields = Vec::new();

    for field in fields {
        properties.push(format!(r#""{}":{}"#, field.name, field.schema_json));
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
}
