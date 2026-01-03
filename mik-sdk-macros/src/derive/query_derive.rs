//! #[derive(Query)] implementation for query parameter types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Type, parse_macro_input};
use utoipa::openapi::{ObjectBuilder, RefOr, Schema, schema::SchemaType};

use super::{
    DeriveContext, extract_named_fields, get_inner_type, is_option_type, parse_field_attrs,
    rust_type_to_name,
};
use crate::openapi::utoipa::{ref_or_schema_to_json, schema_to_json};

// ============================================================================
// DERIVE QUERY
// ============================================================================

#[allow(clippy::too_many_lines)] // Complex derive with many field processing branches
pub fn derive_query_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match extract_named_fields(&input, DeriveContext::Query) {
        Ok(fields) => fields,
        Err(err) => return err,
    };

    let mut field_inits = Vec::new();
    let mut field_matches = Vec::new();
    let mut field_finals = Vec::new();

    // Build the object schema using utoipa ObjectBuilder
    let mut schema_builder = ObjectBuilder::new();

    // Build query parameters array
    let mut query_params_array: Vec<serde_json::Value> = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let attrs = match parse_field_attrs(&field.attrs) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        };

        let query_key = attrs
            .rename
            .clone()
            .unwrap_or_else(|| field_name.to_string());
        let is_optional = is_option_type(field_ty);

        // Get the type name for error messages
        let inner_ty = if is_optional {
            get_inner_type(field_ty)
        } else {
            Some(field_ty)
        };
        let type_name = inner_ty.map_or("value", rust_type_to_name);

        if is_optional {
            field_inits.push(quote! {
                let mut #field_name: #field_ty = None;
            });
            field_matches.push(quote! {
                #query_key => {
                    #field_name = Some(__v.parse().map_err(|_|
                        mik_sdk::typed::ParseError::type_mismatch(#query_key, #type_name)
                    )?);
                }
            });
            field_finals.push(quote! { #field_name });

            // Build schema for this field using utoipa
            let (field_schema, _) = build_query_type_schema(field_ty);
            schema_builder = schema_builder.property(&query_key, field_schema.clone());

            // OpenAPI parameter: optional
            let param = build_query_parameter(&query_key, false, field_schema);
            query_params_array.push(param);
        } else if let Some(ref default) = attrs.default {
            // Has default value
            let default_val: TokenStream2 = default
                .parse()
                .unwrap_or_else(|_| quote! { Default::default() });
            field_inits.push(quote! {
                let mut #field_name: #field_ty = #default_val;
            });
            field_matches.push(quote! {
                #query_key => {
                    #field_name = __v.parse().map_err(|_|
                        mik_sdk::typed::ParseError::type_mismatch(#query_key, #type_name)
                    )?;
                }
            });
            field_finals.push(quote! { #field_name });

            // Try to convert default value to serde_json::Value for OpenAPI
            let default_json_value = default_to_json_value(default);

            // Build schema with optional default value using utoipa
            let field_schema = build_query_type_schema_with_default(field_ty, default_json_value);
            schema_builder = schema_builder.property(&query_key, field_schema.clone());

            // OpenAPI parameter: optional (has default)
            let param = build_query_parameter(&query_key, false, field_schema);
            query_params_array.push(param);
        } else {
            // Required without default
            field_inits.push(quote! {
                let mut #field_name: Option<#field_ty> = None;
            });
            field_matches.push(quote! {
                #query_key => {
                    #field_name = Some(__v.parse().map_err(|_|
                        mik_sdk::typed::ParseError::type_mismatch(#query_key, #type_name)
                    )?);
                }
            });
            field_finals.push(quote! {
                #field_name: #field_name.ok_or_else(|| mik_sdk::typed::ParseError::missing(#query_key))?
            });

            // Build schema for this field using utoipa
            let (field_schema, _) = build_query_type_schema(field_ty);
            schema_builder = schema_builder.property(&query_key, field_schema.clone());
            schema_builder = schema_builder.required(&query_key);

            // OpenAPI parameter: required
            let param = build_query_parameter(&query_key, true, field_schema);
            query_params_array.push(param);
        }
    }

    // Build final schema JSON using utoipa
    let schema: Schema = schema_builder.build().into();
    let schema_json = schema_to_json(&schema);
    let name_str = name.to_string();

    // Build OpenAPI query parameters array JSON
    let query_params_json =
        serde_json::to_string(&query_params_array).unwrap_or_else(|_| "[]".to_string());

    let tokens = quote! {
        impl mik_sdk::typed::FromQuery for #name {
            fn from_query(__params: &[(String, String)]) -> Result<Self, mik_sdk::typed::ParseError> {
                #(#field_inits)*

                for (__k, __v) in __params {
                    match __k.as_str() {
                        #(#field_matches)*
                        _ => {}
                    }
                }

                Ok(Self {
                    #(#field_finals),*
                })
            }
        }

        impl mik_sdk::typed::OpenApiSchema for #name {
            fn openapi_schema() -> &'static str {
                #schema_json
            }

            fn schema_name() -> &'static str {
                #name_str
            }

            fn openapi_query_params() -> &'static str {
                #query_params_json
            }
        }
    };

    TokenStream::from(tokens)
}

/// Try to convert a Rust default value to a `serde_json::Value`.
/// Returns None for complex expressions that can't be represented in JSON.
fn default_to_json_value(default: &str) -> Option<serde_json::Value> {
    let trimmed = default.trim();

    // Boolean literals
    if trimmed == "true" {
        return Some(serde_json::Value::Bool(true));
    }
    if trimmed == "false" {
        return Some(serde_json::Value::Bool(false));
    }

    // Integer literals (including negative)
    if let Ok(n) = trimmed.parse::<i64>() {
        return Some(serde_json::Value::Number(n.into()));
    }

    // Float literals
    if let Ok(n) = trimmed.parse::<f64>() {
        return serde_json::Number::from_f64(n).map(serde_json::Value::Number);
    }

    // String literals: "hello" or 'hello'
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        let inner = &trimmed[1..trimmed.len() - 1];
        return Some(serde_json::Value::String(inner.to_string()));
    }

    // Complex expressions - can't represent in JSON
    None
}

/// Build an OpenAPI schema for a query parameter type using utoipa.
/// Returns a tuple of (schema, is_string_type) where is_string_type is used
/// for constraint application.
fn build_query_type_schema(ty: &Type) -> (RefOr<Schema>, bool) {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    // Handle Option<T> - extract inner type
    let inner_type = if type_str.starts_with("Option<") && type_str.ends_with('>') {
        &type_str[7..type_str.len() - 1]
    } else {
        &type_str
    };

    match inner_type {
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            let schema = ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Integer))
                .build()
                .into();
            (RefOr::T(schema), false)
        },
        "f32" | "f64" => {
            let schema = ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Number))
                .build()
                .into();
            (RefOr::T(schema), false)
        },
        "bool" => {
            let schema = ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::Boolean))
                .build()
                .into();
            (RefOr::T(schema), false)
        },
        // Default to string for String, &str, and unknown types
        _ => {
            let schema = ObjectBuilder::new()
                .schema_type(SchemaType::Type(utoipa::openapi::Type::String))
                .build()
                .into();
            (RefOr::T(schema), true)
        },
    }
}

/// Build an OpenAPI schema for a query parameter type with an optional default value.
fn build_query_type_schema_with_default(
    ty: &Type,
    default_value: Option<serde_json::Value>,
) -> RefOr<Schema> {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    // Handle Option<T> - extract inner type
    let inner_type = if type_str.starts_with("Option<") && type_str.ends_with('>') {
        &type_str[7..type_str.len() - 1]
    } else {
        &type_str
    };

    let mut builder = ObjectBuilder::new();

    match inner_type {
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            builder = builder.schema_type(SchemaType::Type(utoipa::openapi::Type::Integer));
        },
        "f32" | "f64" => {
            builder = builder.schema_type(SchemaType::Type(utoipa::openapi::Type::Number));
        },
        "bool" => {
            builder = builder.schema_type(SchemaType::Type(utoipa::openapi::Type::Boolean));
        },
        // Default to string for String, &str, and unknown types
        _ => {
            builder = builder.schema_type(SchemaType::Type(utoipa::openapi::Type::String));
        },
    }

    if let Some(default_val) = default_value {
        builder = builder.default(Some(default_val));
    }

    RefOr::T(builder.build().into())
}

/// Build an OpenAPI query parameter object as a `serde_json::Value`.
fn build_query_parameter(name: &str, required: bool, schema: RefOr<Schema>) -> serde_json::Value {
    let schema_json: serde_json::Value = serde_json::from_str(&ref_or_schema_to_json(&schema))
        .unwrap_or_else(|_| serde_json::json!({}));

    serde_json::json!({
        "name": name,
        "in": "query",
        "required": required,
        "schema": schema_json
    })
}
