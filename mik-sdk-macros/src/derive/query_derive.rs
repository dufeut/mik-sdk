//! #[derive(Query)] implementation for query parameter types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Type, parse_macro_input};

use super::{
    DeriveContext, escape_json_string, extract_named_fields, get_inner_type, is_option_type,
    parse_field_attrs, rust_type_to_name,
};

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
    let mut schema_props = Vec::new();
    let mut required_fields = Vec::new();
    let mut query_params = Vec::new(); // OpenAPI query parameter objects

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

        // Determine OpenAPI type for this field
        let openapi_type = get_openapi_type_for_query(field_ty);

        // Escape query_key for use in JSON schema output
        let escaped_query_key = escape_json_string(&query_key);

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
            // Optional fields are not required
            schema_props.push(format!(r#""{escaped_query_key}":{openapi_type}"#));
            // OpenAPI parameter: optional
            query_params.push(format!(
                r#"{{"name":"{escaped_query_key}","in":"query","required":false,"schema":{openapi_type}}}"#
            ));
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
            // Fields with defaults are not required
            schema_props.push(format!(r#""{escaped_query_key}":{openapi_type}"#));
            // OpenAPI parameter: has default, not required
            query_params.push(format!(
                r#"{{"name":"{escaped_query_key}","in":"query","required":false,"schema":{openapi_type}}}"#
            ));
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
            // Required field
            schema_props.push(format!(r#""{escaped_query_key}":{openapi_type}"#));
            required_fields.push(format!(r#""{escaped_query_key}""#));
            // OpenAPI parameter: required
            query_params.push(format!(
                r#"{{"name":"{escaped_query_key}","in":"query","required":true,"schema":{openapi_type}}}"#
            ));
        }
    }

    let schema_props_str = schema_props.join(",");
    let required_str = required_fields.join(",");
    let schema_json = if required_fields.is_empty() {
        format!(r#"{{"type":"object","properties":{{{schema_props_str}}}}}"#)
    } else {
        format!(
            r#"{{"type":"object","properties":{{{schema_props_str}}},"required":[{required_str}]}}"#
        )
    };
    let name_str = name.to_string();

    // Build OpenAPI query parameters array
    let query_params_json = format!("[{}]", query_params.join(","));

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

/// Get OpenAPI type string for query parameter types
fn get_openapi_type_for_query(ty: &Type) -> String {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    // Handle Option<T> - extract inner type
    let inner_type = if type_str.starts_with("Option<") && type_str.ends_with('>') {
        &type_str[7..type_str.len() - 1]
    } else {
        &type_str
    };

    match inner_type {
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            r#"{"type":"integer"}"#.to_string()
        },
        "f32" | "f64" => r#"{"type":"number"}"#.to_string(),
        "bool" => r#"{"type":"boolean"}"#.to_string(),
        // Default to string for String, &str, and unknown types
        _ => r#"{"type":"string"}"#.to_string(),
    }
}
