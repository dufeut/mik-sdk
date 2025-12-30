//! #[derive(Type)] implementation for JSON body/response types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use super::{
    DeriveContext, escape_json_string, extract_named_fields, get_inner_type, is_option_type,
    parse_field_attrs, rust_type_to_json_getter, rust_type_to_name, type_to_openapi,
};

// ============================================================================
// DERIVE TYPE
// ============================================================================

#[allow(clippy::too_many_lines)] // Complex derive with many type handling branches
pub fn derive_type_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    let fields = match extract_named_fields(&input, DeriveContext::Type) {
        Ok(fields) => fields,
        Err(err) => return err,
    };

    // Generate from_json implementation
    let mut from_json_fields = Vec::new();
    let mut required_fields = Vec::new();
    let mut openapi_properties = Vec::new();
    let mut validation_checks = Vec::new();

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
                    format!("Cannot extract inner type from Option for field '{field_name_str}'. Use a concrete type like Option<String> instead of Option<impl Trait>.")
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
            required_fields.push(json_key.clone());
        }

        // Generate OpenAPI property
        let mut base_schema = type_to_openapi(field_ty);

        // Add constraints to schema
        let mut extra_props = Vec::new();
        if let Some(min) = attrs.min {
            if base_schema.contains("string") {
                extra_props.push(format!(r#""minLength":{min}"#));
            } else if base_schema.contains("integer") || base_schema.contains("number") {
                extra_props.push(format!(r#""minimum":{min}"#));
            } else if base_schema.contains("array") {
                extra_props.push(format!(r#""minItems":{min}"#));
            }
        }
        if let Some(max) = attrs.max {
            if base_schema.contains("string") {
                extra_props.push(format!(r#""maxLength":{max}"#));
            } else if base_schema.contains("integer") || base_schema.contains("number") {
                extra_props.push(format!(r#""maximum":{max}"#));
            } else if base_schema.contains("array") {
                extra_props.push(format!(r#""maxItems":{max}"#));
            }
        }
        if let Some(ref fmt) = attrs.format {
            extra_props.push(format!(r#""format":"{}""#, escape_json_string(fmt)));
        }
        if let Some(ref pattern) = attrs.pattern {
            extra_props.push(format!(r#""pattern":"{}""#, escape_json_string(pattern)));
        }
        if let Some(ref docs) = attrs.docs {
            extra_props.push(format!(r#""description":"{}""#, escape_json_string(docs)));
        }

        if !extra_props.is_empty() {
            // Merge extra props into base schema (safe extraction)
            let base_inner = base_schema
                .strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .unwrap_or(&base_schema);
            base_schema = format!("{{{},{}}}", base_inner, extra_props.join(","));
        }

        openapi_properties.push(format!(
            r#""{}":{}"#,
            escape_json_string(&json_key),
            base_schema
        ));

        // Generate validation checks
        generate_validation_checks(
            &attrs,
            field_name,
            is_optional,
            &base_schema,
            &mut validation_checks,
        );
    }

    // Build OpenAPI schema
    let required_json = if required_fields.is_empty() {
        String::new()
    } else {
        format!(
            r#","required":[{}]"#,
            required_fields
                .iter()
                .map(|f| format!(r#""{}""#, escape_json_string(f)))
                .collect::<Vec<_>>()
                .join(",")
        )
    };

    let openapi_schema = format!(
        r#"{{"type":"object","properties":{{{}}}{}}}  "#,
        openapi_properties.join(","),
        required_json
    );

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

/// Generate validation check code for a field
fn generate_validation_checks(
    attrs: &super::FieldAttrs,
    field_name: &syn::Ident,
    is_optional: bool,
    base_schema: &str,
    validation_checks: &mut Vec<TokenStream2>,
) {
    if let Some(min) = attrs.min {
        let field_name_str = field_name.to_string();
        if is_optional {
            // Validate optional fields when Some
            if base_schema.contains("string") {
                validation_checks.push(quote! {
                    if let Some(ref __val) = self.#field_name {
                        if __val.len() < #min as usize {
                            return Err(mik_sdk::typed::ValidationError::min(#field_name_str, #min));
                        }
                    }
                });
            } else {
                // Use i128 for safe comparison across all integer types (avoids u64 -> i64 overflow)
                validation_checks.push(quote! {
                    if let Some(__val) = self.#field_name {
                        if (__val as i128) < (#min as i128) {
                            return Err(mik_sdk::typed::ValidationError::min(#field_name_str, #min));
                        }
                    }
                });
            }
        } else if base_schema.contains("string") {
            validation_checks.push(quote! {
                if self.#field_name.len() < #min as usize {
                    return Err(mik_sdk::typed::ValidationError::min(#field_name_str, #min));
                }
            });
        } else {
            // Use i128 for safe comparison across all integer types (avoids u64 -> i64 overflow)
            validation_checks.push(quote! {
                if (self.#field_name as i128) < (#min as i128) {
                    return Err(mik_sdk::typed::ValidationError::min(#field_name_str, #min));
                }
            });
        }
    }
    if let Some(max) = attrs.max {
        let field_name_str = field_name.to_string();
        if is_optional {
            // Validate optional fields when Some
            if base_schema.contains("string") {
                validation_checks.push(quote! {
                    if let Some(ref __val) = self.#field_name {
                        if __val.len() > #max as usize {
                            return Err(mik_sdk::typed::ValidationError::max(#field_name_str, #max));
                        }
                    }
                });
            } else {
                // Use i128 for safe comparison across all integer types (avoids u64 -> i64 overflow)
                validation_checks.push(quote! {
                    if let Some(__val) = self.#field_name {
                        if (__val as i128) > (#max as i128) {
                            return Err(mik_sdk::typed::ValidationError::max(#field_name_str, #max));
                        }
                    }
                });
            }
        } else if base_schema.contains("string") {
            validation_checks.push(quote! {
                if self.#field_name.len() > #max as usize {
                    return Err(mik_sdk::typed::ValidationError::max(#field_name_str, #max));
                }
            });
        } else {
            // Use i128 for safe comparison across all integer types (avoids u64 -> i64 overflow)
            validation_checks.push(quote! {
                if (self.#field_name as i128) > (#max as i128) {
                    return Err(mik_sdk::typed::ValidationError::max(#field_name_str, #max));
                }
            });
        }
    }
}
