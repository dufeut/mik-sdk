//! #[derive(Type)] implementation for JSON body/response types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

use super::{
    escape_json_string, get_inner_type, is_option_type, parse_field_attrs,
    rust_type_to_json_getter, rust_type_to_name, type_to_openapi,
};

// ============================================================================
// HELPER: PascalCase to snake_case conversion
// ============================================================================

/// Convert PascalCase to snake_case.
///
/// Examples:
/// - `Active` → `active`
/// - `SuperAdmin` → `super_admin`
/// - `HTTPRequest` → `http_request`
fn pascal_to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut prev_was_upper = false;
    let mut prev_was_underscore = true; // Start as true to avoid leading underscore

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            // Add underscore before uppercase if:
            // - Not at start
            // - Previous char wasn't uppercase (handles "HTTPRequest" → "http_request")
            // - OR next char is lowercase (handles "XMLParser" → "xml_parser")
            let next_is_lower = s.chars().nth(i + 1).is_some_and(char::is_lowercase);
            if !prev_was_underscore && (!prev_was_upper || next_is_lower) {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_upper = true;
            prev_was_underscore = false;
        } else if c == '_' {
            result.push(c);
            prev_was_upper = false;
            prev_was_underscore = true;
        } else {
            result.push(c);
            prev_was_upper = false;
            prev_was_underscore = false;
        }
    }

    result
}

// ============================================================================
// DERIVE TYPE - Entry Point
// ============================================================================

#[allow(clippy::too_many_lines)] // Complex derive with many type handling branches
#[allow(clippy::cognitive_complexity)] // Complex macro with many type branches
pub fn derive_type_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Check if this is an enum or struct
    match &input.data {
        Data::Enum(data_enum) => derive_enum_type_impl(&input, data_enum),
        Data::Struct(data_struct) => derive_struct_type_impl(&input, data_struct),
        Data::Union(_) => syn::Error::new_spanned(
            &input,
            "Oops! #[derive(Type)] only works on structs and enums, not unions.\n\
             \n\
             ✅ What you need:\n\
               #[derive(Type)]\n\
               struct MyType { field: String }\n\
             \n\
               // Or for enums:\n\
               #[derive(Type)]\n\
               enum Status { Active, Inactive }",
        )
        .to_compile_error()
        .into(),
    }
}

// ============================================================================
// DERIVE TYPE - Enum Implementation
// ============================================================================

#[allow(clippy::too_many_lines)]
fn derive_enum_type_impl(input: &DeriveInput, data_enum: &syn::DataEnum) -> TokenStream {
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
                     ❌ What you have:\n\
                       {}(..)\n\
                     \n\
                     ✅ What you need:\n\
                       {}\n\
                     \n\
                     For complex enums with data, consider using separate types.",
                    variant.ident, variant.ident
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
                Self::#ident => ::mik_sdk::json::str(#json_name),
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

    // Generate OpenAPI schema with enum array
    let enum_values_json = valid_values
        .iter()
        .map(|v| format!("\"{}\"", escape_json_string(v)))
        .collect::<Vec<_>>()
        .join(",");
    let openapi_schema = format!(r#"{{"type":"string","enum":[{enum_values_json}]}}"#);

    let tokens = quote! {
        impl ::mik_sdk::typed::FromJson for #name {
            fn from_json(__value: &::mik_sdk::json::JsonValue) -> Result<Self, ::mik_sdk::typed::ParseError> {
                let __s = __value.str().ok_or_else(|| {
                    ::mik_sdk::typed::ParseError::type_mismatch("value", "string")
                })?;

                match __s.as_str() {
                    #(#from_json_arms)*
                    __other => Err(::mik_sdk::typed::ParseError::custom(
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

        impl ::mik_sdk::json::ToJson for #name {
            fn to_json(&self) -> ::mik_sdk::json::JsonValue {
                match self {
                    #(#to_json_arms)*
                }
            }
        }

        impl ::mik_sdk::typed::Validate for #name {
            fn validate(&self) -> Result<(), ::mik_sdk::typed::ValidationError> {
                // Enums are always valid if parsed successfully
                Ok(())
            }
        }

        impl ::mik_sdk::typed::OpenApiSchema for #name {
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

// ============================================================================
// DERIVE TYPE - Struct Implementation
// ============================================================================

#[allow(clippy::too_many_lines)]
fn derive_struct_type_impl(input: &DeriveInput, data_struct: &syn::DataStruct) -> TokenStream {
    let name = &input.ident;
    let name_str = name.to_string();

    let fields = match &data_struct.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return syn::Error::new_spanned(
                input,
                "Oops! #[derive(Type)] needs named fields, not tuple fields.\n\
                 \n\
                 ❌ What you have (tuple struct):\n\
                   struct MyStruct(String, i32);\n\
                 \n\
                 ✅ What you need (named fields):\n\
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
                 ❌ What you have (empty struct):\n\
                   struct MyStruct;\n\
                 \n\
                 ✅ What you need:\n\
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
                    format!(
                        "Can't figure out what type is inside Option for '{field_name_str}'.\n\
                         \n\
                         ✅ Use a concrete type:\n\
                         pub {field_name_str}: Option<String>,     // text\n\
                         pub {field_name_str}: Option<i32>,        // number\n\
                         pub {field_name_str}: Option<bool>,       // true/false\n\
                         pub {field_name_str}: Option<MyStruct>,   // your own type\n\
                         \n\
                         ❌ These won't work:\n\
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

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::pascal_to_snake_case;

    #[test]
    fn test_pascal_to_snake_case_simple() {
        assert_eq!(pascal_to_snake_case("Active"), "active");
        assert_eq!(pascal_to_snake_case("Inactive"), "inactive");
        assert_eq!(pascal_to_snake_case("Pending"), "pending");
    }

    #[test]
    fn test_pascal_to_snake_case_multi_word() {
        assert_eq!(pascal_to_snake_case("SuperAdmin"), "super_admin");
        assert_eq!(pascal_to_snake_case("RegularUser"), "regular_user");
        assert_eq!(pascal_to_snake_case("GuestUser"), "guest_user");
    }

    #[test]
    fn test_pascal_to_snake_case_acronyms() {
        assert_eq!(pascal_to_snake_case("HTTPRequest"), "http_request");
        assert_eq!(pascal_to_snake_case("XMLParser"), "xml_parser");
        assert_eq!(pascal_to_snake_case("APIResponse"), "api_response");
    }

    #[test]
    fn test_pascal_to_snake_case_single_letter() {
        assert_eq!(pascal_to_snake_case("A"), "a");
        assert_eq!(pascal_to_snake_case("AB"), "ab");
    }

    #[test]
    fn test_pascal_to_snake_case_already_lower() {
        assert_eq!(pascal_to_snake_case("active"), "active");
        assert_eq!(pascal_to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn test_pascal_to_snake_case_numbers() {
        assert_eq!(pascal_to_snake_case("Status2"), "status2");
        assert_eq!(pascal_to_snake_case("OAuth2Token"), "o_auth2_token");
    }
}
