//! Struct implementation for #[derive(Type)].

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Fields};

use super::validation::generate_validation_checks;
use crate::derive::{
    get_inner_type, is_option_type, parse_field_attrs, rust_type_to_json_getter, rust_type_to_name,
};
use crate::openapi::utoipa::{
    FieldConstraints, JsonFieldDef, apply_constraints, object_schema_json, schema_to_json,
};
use crate::type_registry::get_openapi_schema;

/// Generate FromJson, Validate, and OpenApiSchema implementations for structs.
#[allow(clippy::too_many_lines)]
pub fn derive_struct_type_impl(input: &DeriveInput, data_struct: &syn::DataStruct) -> TokenStream {
    let name = &input.ident;
    let name_str = name.to_string();

    let fields = match &data_struct.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return syn::Error::new_spanned(
                input,
                "Oops! #[derive(Type)] needs named fields, not tuple fields.\n\
                 \n\
                 \u{274C} What you have (tuple struct):\n\
                   struct MyStruct(String, i32);\n\
                 \n\
                 \u{2705} What you need (named fields):\n\
                   #[derive(Type)]\n\
                   struct MyType { field: String }\n\
                 \n\
                 Named fields have names like 'id', 'name', 'email' - they make\n\
                 your code easier to read and work with JSON/query params.",
            )
            .to_compile_error()
            .into();
        },
        Fields::Unit => {
            return syn::Error::new_spanned(
                input,
                "Oops! #[derive(Type)] needs a struct with fields.\n\
                 \n\
                 \u{274C} What you have (empty struct):\n\
                   struct MyStruct;\n\
                 \n\
                 \u{2705} What you need:\n\
                   #[derive(Type)]\n\
                   struct MyType { field: String }\n\
                 \n\
                 Add some fields to hold your data!",
            )
            .to_compile_error()
            .into();
        },
    };

    // Generate from_json implementation
    let mut from_json_fields = Vec::new();
    let mut field_defs: Vec<JsonFieldDef> = Vec::new();
    let mut validation_checks: Vec<TokenStream2> = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let attrs = match parse_field_attrs(&field.attrs) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        };

        let json_key = attrs
            .rename
            .clone()
            .unwrap_or_else(|| field_name.to_string());
        let is_optional = is_option_type(field_ty);

        // Generate from_json field extraction
        if is_optional {
            let inner_ty = get_inner_type(field_ty);
            let inner_getter = inner_ty.and_then(rust_type_to_json_getter);

            if let Some(getter) = inner_getter {
                // Simple type inside Option - use getter method
                let type_name = inner_ty.map_or("value", rust_type_to_name);
                from_json_fields.push(quote! {
                    #field_name: {
                        let v = __value.get(#json_key);
                        if v.is_null() {
                            None
                        } else {
                            Some(v #getter .ok_or_else(|| mik_sdk::typed::ParseError::type_mismatch(#json_key, #type_name))?)
                        }
                    }
                });
            } else if let Some(inner) = inner_ty {
                // Complex type inside Option - use FromJson trait
                from_json_fields.push(quote! {
                    #field_name: {
                        let v = __value.get(#json_key);
                        if v.is_null() {
                            None
                        } else {
                            Some(<#inner as mik_sdk::typed::FromJson>::from_json(&v)?)
                        }
                    }
                });
            } else {
                // Could not extract inner type from Option - emit compile error
                let field_name_str = field_name.to_string();
                return syn::Error::new_spanned(
                    field_ty,
                    format!(
                        "Can't figure out what type is inside Option for '{field_name_str}'.\n\
                         \n\
                         \u{2705} Use a concrete type:\n\
                         pub {field_name_str}: Option<String>,     // text\n\
                         pub {field_name_str}: Option<i32>,        // number\n\
                         pub {field_name_str}: Option<bool>,       // true/false\n\
                         pub {field_name_str}: Option<MyStruct>,   // your own type\n\
                         \n\
                         \u{274C} These won't work:\n\
                         pub {field_name_str}: Option<impl Trait>, // too abstract\n\
                         pub {field_name_str}: Option<_>,          // can't infer type"
                    ),
                )
                .to_compile_error()
                .into();
            }
        } else {
            let getter = rust_type_to_json_getter(field_ty);
            if let Some(getter) = getter {
                // Simple type - use getter method
                from_json_fields.push(quote! {
                    #field_name: __value.get(#json_key) #getter
                        .ok_or_else(|| mik_sdk::typed::ParseError::missing(#json_key))?
                });
            } else {
                // Complex type (Vec, custom struct, etc.) - use FromJson trait
                from_json_fields.push(quote! {
                    #field_name: <#field_ty as mik_sdk::typed::FromJson>::from_json(&__value.get(#json_key))?
                });
            }
        }

        // Generate OpenAPI property using utoipa
        // First, get the base schema JSON (used for validation type detection)
        let base_schema_json = get_openapi_schema(field_ty);

        // Build constraints from field attributes
        let constraints = FieldConstraints {
            min: attrs.min,
            max: attrs.max,
            format: attrs.format.clone(),
            pattern: attrs.pattern.clone(),
            description: attrs.docs.clone(),
            x_attrs: attrs.x_attrs.clone(),
        };

        // Determine if this is a string type for constraint application
        let is_string_type = base_schema_json.contains("\"type\":\"string\"");
        let is_array_type = base_schema_json.contains("\"type\":\"array\"");

        // Build the field schema with constraints applied
        let field_schema = if constraints.min.is_some()
            || constraints.max.is_some()
            || constraints.format.is_some()
            || constraints.pattern.is_some()
            || constraints.description.is_some()
        {
            // Apply constraints using utoipa ObjectBuilder
            build_schema_with_constraints(field_ty, &constraints, is_string_type, is_array_type)
        } else {
            // No constraints - use the base schema directly
            base_schema_json.clone()
        };

        // Add field definition for object_schema_json (preserves nullable)
        field_defs.push(JsonFieldDef {
            name: json_key.clone(),
            schema_json: field_schema,
            required: !is_optional,
            x_attrs: attrs.x_attrs.clone(),
        });

        // Generate validation checks (still uses base_schema_json for type detection)
        generate_validation_checks(
            &attrs,
            field_name,
            is_optional,
            &base_schema_json,
            &mut validation_checks,
        );
    }

    // Build OpenAPI schema using JSON-based helper (preserves nullable)
    let openapi_schema = object_schema_json(field_defs);

    let tokens = quote! {
        impl mik_sdk::typed::FromJson for #name {
            fn from_json(__value: &mik_sdk::json::JsonValue) -> Result<Self, mik_sdk::typed::ParseError> {
                Ok(Self {
                    #(#from_json_fields),*
                })
            }
        }

        impl mik_sdk::typed::Validate for #name {
            fn validate(&self) -> Result<(), mik_sdk::typed::ValidationError> {
                #(#validation_checks)*
                Ok(())
            }
        }

        impl mik_sdk::typed::OpenApiSchema for #name {
            fn openapi_schema() -> &'static str {
                #openapi_schema
            }

            fn schema_name() -> &'static str {
                #name_str
            }
        }
    };

    TokenStream::from(tokens)
}

/// Build a schema with constraints applied using utoipa.
///
/// This function handles applying min/max/format/pattern/description constraints
/// to a field schema. It uses utoipa's `ObjectBuilder` for type-safe schema construction.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn build_schema_with_constraints(
    ty: &syn::Type,
    constraints: &FieldConstraints,
    is_string: bool,
    is_array: bool,
) -> String {
    use utoipa::openapi::{ArrayBuilder, ObjectBuilder};

    // Get base schema JSON and parse the type
    let base_json = get_openapi_schema(ty);

    // For arrays, we need to handle minItems/maxItems specially
    if is_array {
        // Parse and rebuild with array constraints
        let inner_ty = get_inner_type(ty);
        let items_schema = inner_ty.map_or_else(
            || {
                ObjectBuilder::new()
                    .schema_type(utoipa::openapi::schema::SchemaType::Type(
                        utoipa::openapi::Type::Object,
                    ))
                    .build()
                    .into()
            },
            |inner| {
                let inner_json = get_openapi_schema(inner);
                serde_json::from_str(&inner_json).unwrap_or_else(|_| {
                    utoipa::openapi::RefOr::T(
                        ObjectBuilder::new()
                            .schema_type(utoipa::openapi::schema::SchemaType::Type(
                                utoipa::openapi::Type::Object,
                            ))
                            .build()
                            .into(),
                    )
                })
            },
        );

        let mut arr_builder = ArrayBuilder::new().items(items_schema);

        if let Some(min) = constraints.min {
            arr_builder = arr_builder.min_items(Some(min as usize));
        }
        if let Some(max) = constraints.max {
            arr_builder = arr_builder.max_items(Some(max as usize));
        }

        let arr_schema: utoipa::openapi::Schema = arr_builder.build().into();
        return schema_to_json(&arr_schema);
    }

    // Check if this is a nullable (Option) type
    let is_nullable = base_json.contains("\"nullable\":true");

    // For $ref types, we can't easily add constraints via utoipa
    if base_json.contains("\"$ref\"") {
        return base_json;
    }

    // For non-array types, use ObjectBuilder with apply_constraints
    let mut builder = ObjectBuilder::new();

    // Set the type based on base schema
    if base_json.contains("\"type\":\"string\"") {
        builder = builder.schema_type(utoipa::openapi::schema::SchemaType::Type(
            utoipa::openapi::Type::String,
        ));
    } else if base_json.contains("\"type\":\"integer\"") {
        builder = builder.schema_type(utoipa::openapi::schema::SchemaType::Type(
            utoipa::openapi::Type::Integer,
        ));
    } else if base_json.contains("\"type\":\"number\"") {
        builder = builder.schema_type(utoipa::openapi::schema::SchemaType::Type(
            utoipa::openapi::Type::Number,
        ));
    } else if base_json.contains("\"type\":\"boolean\"") {
        builder = builder.schema_type(utoipa::openapi::schema::SchemaType::Type(
            utoipa::openapi::Type::Boolean,
        ));
    } else {
        builder = builder.schema_type(utoipa::openapi::schema::SchemaType::Type(
            utoipa::openapi::Type::Object,
        ));
    }

    // Apply constraints using the utoipa helper
    builder = apply_constraints(builder, constraints, is_string);

    let schema: utoipa::openapi::Schema = builder.build().into();
    let schema_json = schema_to_json(&schema);

    // Re-add nullable if this was an Option type
    if is_nullable {
        crate::openapi::utoipa::make_nullable_json(&schema_json)
    } else {
        schema_json
    }
}
