//! Enum implementation for #[derive(Type)].

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields};

use super::case::pascal_to_snake_case;
use crate::derive::parse_field_attrs;
use crate::openapi::utoipa::{enum_schema, schema_to_json};

/// Generate FromJson, ToJson, Validate, and OpenApiSchema implementations for enums.
#[allow(clippy::too_many_lines)]
pub fn derive_enum_type_impl(input: &DeriveInput, data_enum: &syn::DataEnum) -> TokenStream {
    let name = &input.ident;
    let name_str = name.to_string();

    // Collect variant info: (variant_ident, json_name)
    let mut variants_info: Vec<(&syn::Ident, String)> = Vec::new();

    for variant in &data_enum.variants {
        // Only support unit variants (no fields)
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                variant,
                format!(
                    "Enum variants must be unit variants (no fields).\n\
                     \n\
                     {} What you have:\n\
                       {}(..)\n\
                     \n\
                     {} What you need:\n\
                       {}\n\
                     \n\
                     For complex enums with data, consider using separate types.",
                    '\u{274C}', // X mark
                    variant.ident,
                    '\u{2705}', // checkmark
                    variant.ident
                ),
            )
            .to_compile_error()
            .into();
        }

        // Check for #[field(rename = "...")] attribute
        let attrs = match parse_field_attrs(&variant.attrs) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        };

        let json_name = attrs
            .rename
            .unwrap_or_else(|| pascal_to_snake_case(&variant.ident.to_string()));

        variants_info.push((&variant.ident, json_name));
    }

    // Generate FromJson match arms
    let from_json_arms: Vec<_> = variants_info
        .iter()
        .map(|(ident, json_name)| {
            quote! {
                #json_name => Ok(Self::#ident),
            }
        })
        .collect();

    // Generate ToJson match arms
    let to_json_arms: Vec<_> = variants_info
        .iter()
        .map(|(ident, json_name)| {
            quote! {
                Self::#ident => mik_sdk::json::str(#json_name),
            }
        })
        .collect();

    // Generate valid values list for error message
    let valid_values: Vec<_> = variants_info
        .iter()
        .map(|(_, name)| name.as_str())
        .collect();
    let valid_values_str = valid_values
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");

    // Generate OpenAPI schema with enum array using utoipa
    let schema = enum_schema(&valid_values);
    let openapi_schema = schema_to_json(&schema);

    let tokens = quote! {
        impl mik_sdk::typed::FromJson for #name {
            fn from_json(__value: &mik_sdk::json::JsonValue) -> Result<Self, mik_sdk::typed::ParseError> {
                let __s = __value.str().ok_or_else(|| {
                    mik_sdk::typed::ParseError::type_mismatch("value", "string")
                })?;

                match __s.as_str() {
                    #(#from_json_arms)*
                    __other => Err(mik_sdk::typed::ParseError::custom(
                        "value",
                        format!(
                            "unknown enum variant \"{}\". Valid values: {}",
                            __other,
                            #valid_values_str
                        )
                    )),
                }
            }
        }

        impl mik_sdk::json::ToJson for #name {
            fn to_json(&self) -> mik_sdk::json::JsonValue {
                match self {
                    #(#to_json_arms)*
                }
            }
        }

        impl mik_sdk::typed::Validate for #name {
            fn validate(&self) -> Result<(), mik_sdk::typed::ValidationError> {
                // Enums are always valid if parsed successfully
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
