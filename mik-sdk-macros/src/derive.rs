//! Derive macros for typed inputs: Type, Query, Path.
//!
//! These generate implementations for FromJson, FromQuery, FromPath traits,
//! along with OpenAPI schema generation and optional validation.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, Lit, Type, parse_macro_input};

// ============================================================================
// JSON STRING ESCAPING
// ============================================================================

/// Escape a string for use in JSON output.
/// Handles all JSON control characters per RFC 8259.
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // Control characters (U+0000 to U+001F)
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            },
            c => result.push(c),
        }
    }
    result
}

// ============================================================================
// FIELD ATTRIBUTE PARSING
// ============================================================================

#[derive(Default, Clone)]
struct FieldAttrs {
    min: Option<i64>,
    max: Option<i64>,
    format: Option<String>,
    pattern: Option<String>,
    default: Option<String>,
    rename: Option<String>,
    docs: Option<String>,
}

fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut result = FieldAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("min") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Int(lit) = value {
                    result.min = lit.base10_parse().ok();
                }
            } else if meta.path.is_ident("max") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Int(lit) = value {
                    result.max = lit.base10_parse().ok();
                }
            } else if meta.path.is_ident("format") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(lit) = value {
                    result.format = Some(lit.value());
                }
            } else if meta.path.is_ident("pattern") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(lit) = value {
                    result.pattern = Some(lit.value());
                }
            } else if meta.path.is_ident("default") {
                let value: Expr = meta.value()?.parse()?;
                result.default = Some(quote!(#value).to_string());
            } else if meta.path.is_ident("rename") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(lit) = value {
                    result.rename = Some(lit.value());
                }
            } else if meta.path.is_ident("docs") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(lit) = value {
                    result.docs = Some(lit.value());
                }
            }
            Ok(())
        })?;
    }

    Ok(result)
}

// ============================================================================
// TYPE HELPERS
// ============================================================================

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}

fn get_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

fn type_to_openapi(ty: &Type) -> String {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        let name = segment.ident.to_string();
        return match name.as_str() {
            "String" | "str" => r#"{"type":"string"}"#.to_string(),
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" => {
                r#"{"type":"integer"}"#.to_string()
            },
            "f32" | "f64" => r#"{"type":"number"}"#.to_string(),
            "bool" => r#"{"type":"boolean"}"#.to_string(),
            "Option" => {
                if let Some(inner) = get_inner_type(ty) {
                    let inner_schema = type_to_openapi(inner);
                    // Remove the outer braces and add nullable (safe extraction)
                    let inner_content = inner_schema
                        .strip_prefix('{')
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(&inner_schema);
                    format!("{{{},\"nullable\":true}}", inner_content)
                } else {
                    r#"{"type":"object","nullable":true}"#.to_string()
                }
            },
            "Vec" => {
                if let Some(inner) = get_inner_type(ty) {
                    let inner_schema = type_to_openapi(inner);
                    format!(r#"{{"type":"array","items":{}}}"#, inner_schema)
                } else {
                    r#"{"type":"array"}"#.to_string()
                }
            },
            _ => {
                // Assume it's a reference to another schema
                format!("{{\"$ref\":\"#/components/schemas/{}\"}}", name)
            },
        };
    }
    r#"{"type":"object"}"#.to_string()
}

fn rust_type_to_json_getter(ty: &Type) -> Option<TokenStream2> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        let name = segment.ident.to_string();
        return match name.as_str() {
            "String" => Some(quote! { .str() }),
            "i8" | "i16" | "i32" => Some(quote! { .int().map(|n| n as _) }),
            "i64" => Some(quote! { .int() }),
            "u8" | "u16" | "u32" => Some(quote! { .int().map(|n| n as _) }),
            "u64" | "usize" => Some(quote! { .int().map(|n| n as _) }),
            "f32" => Some(quote! { .float().map(|n| n as f32) }),
            "f64" => Some(quote! { .float() }),
            "bool" => Some(quote! { .bool() }),
            _ => None, // Complex type - use FromJson trait
        };
    }
    None
}

/// Get a human-readable type name for error messages
fn rust_type_to_name(ty: &Type) -> &'static str {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        let name = segment.ident.to_string();
        return match name.as_str() {
            "String" => "string",
            "i8" | "i16" | "i32" | "i64" => "integer",
            "u8" | "u16" | "u32" | "u64" | "usize" => "integer",
            "f32" | "f64" => "number",
            "bool" => "boolean",
            "Vec" => "array",
            "Option" => "value",
            _ => "object",
        };
    }
    "value"
}

// ============================================================================
// STRUCT FIELD EXTRACTION HELPER
// ============================================================================

/// Context for derive macro error messages
enum DeriveContext {
    Type,
    Query,
    Path,
}

impl DeriveContext {
    fn name(&self) -> &'static str {
        match self {
            DeriveContext::Type => "Type",
            DeriveContext::Query => "Query",
            DeriveContext::Path => "Path",
        }
    }

    fn example(&self) -> &'static str {
        match self {
            DeriveContext::Type => "struct MyType { field: String }",
            DeriveContext::Query => "struct MyQuery { page: u32, limit: u32 }",
            DeriveContext::Path => "struct UserPath { org_id: String, id: String }",
        }
    }

    fn purpose(&self) -> &'static str {
        match self {
            DeriveContext::Type => "for JSON body/response types",
            DeriveContext::Query => "for query parameters",
            DeriveContext::Path => "for URL path parameters",
        }
    }
}

/// Extract named fields from a DeriveInput, returning an error TokenStream if invalid.
fn extract_named_fields(
    input: &DeriveInput,
    ctx: DeriveContext,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            _ => Err(syn::Error::new_spanned(
                input,
                format!(
                    "{} derive only supports structs with named fields. \
                     Example: `{}`",
                    ctx.name(),
                    ctx.example()
                ),
            )
            .to_compile_error()
            .into()),
        },
        _ => Err(syn::Error::new_spanned(
            input,
            format!(
                "{} derive only supports structs. \
                 Hint: Use `#[derive({})]` on a struct {}.",
                ctx.name(),
                ctx.name(),
                ctx.purpose()
            ),
        )
        .to_compile_error()
        .into()),
    }
}

// ============================================================================
// DERIVE TYPE
// ============================================================================

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
                let type_name = inner_ty.map(rust_type_to_name).unwrap_or("value");
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
                    format!("Cannot extract inner type from Option for field '{}'. Use a concrete type like Option<String> instead of Option<impl Trait>.", field_name_str)
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
                extra_props.push(format!(r#""minLength":{}"#, min));
            } else if base_schema.contains("integer") || base_schema.contains("number") {
                extra_props.push(format!(r#""minimum":{}"#, min));
            } else if base_schema.contains("array") {
                extra_props.push(format!(r#""minItems":{}"#, min));
            }
        }
        if let Some(max) = attrs.max {
            if base_schema.contains("string") {
                extra_props.push(format!(r#""maxLength":{}"#, max));
            } else if base_schema.contains("integer") || base_schema.contains("number") {
                extra_props.push(format!(r#""maximum":{}"#, max));
            } else if base_schema.contains("array") {
                extra_props.push(format!(r#""maxItems":{}"#, max));
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

// ============================================================================
// DERIVE QUERY
// ============================================================================

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
        let type_name = inner_ty.map(rust_type_to_name).unwrap_or("value");

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
            schema_props.push(format!(r#""{}":{}"#, escaped_query_key, openapi_type));
            // OpenAPI parameter: optional
            query_params.push(format!(
                r#"{{"name":"{}","in":"query","required":false,"schema":{}}}"#,
                escaped_query_key, openapi_type
            ));
        } else if let Some(ref default) = attrs.default {
            // Has default value
            let default_val: TokenStream2 =
                default.parse().unwrap_or(quote! { Default::default() });
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
            schema_props.push(format!(r#""{}":{}"#, escaped_query_key, openapi_type));
            // OpenAPI parameter: has default, not required
            query_params.push(format!(
                r#"{{"name":"{}","in":"query","required":false,"schema":{}}}"#,
                escaped_query_key, openapi_type
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
            schema_props.push(format!(r#""{}":{}"#, escaped_query_key, openapi_type));
            required_fields.push(format!(r#""{}""#, escaped_query_key));
            // OpenAPI parameter: required
            query_params.push(format!(
                r#"{{"name":"{}","in":"query","required":true,"schema":{}}}"#,
                escaped_query_key, openapi_type
            ));
        }
    }

    let schema_props_str = schema_props.join(",");
    let required_str = required_fields.join(",");
    let schema_json = if required_fields.is_empty() {
        format!(
            r#"{{"type":"object","properties":{{{}}}}}"#,
            schema_props_str
        )
    } else {
        format!(
            r#"{{"type":"object","properties":{{{}}},"required":[{}]}}"#,
            schema_props_str, required_str
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
        "String" | "&str" => r#"{"type":"string"}"#.to_string(),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            r#"{"type":"integer"}"#.to_string()
        },
        "f32" | "f64" => r#"{"type":"number"}"#.to_string(),
        "bool" => r#"{"type":"boolean"}"#.to_string(),
        _ => r#"{"type":"string"}"#.to_string(), // Default to string for unknown types
    }
}

// ============================================================================
// DERIVE PATH
// ============================================================================

pub fn derive_path_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match extract_named_fields(&input, DeriveContext::Path) {
        Ok(fields) => fields,
        Err(err) => return err,
    };

    let mut field_extractions = Vec::new();
    let mut schema_props = Vec::new();
    let mut required_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let attrs = match parse_field_attrs(&field.attrs) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        };

        let path_key = attrs
            .rename
            .clone()
            .unwrap_or_else(|| field_name.to_string());

        // Check if type is String (direct clone) or needs parsing
        let is_string = if let Type::Path(type_path) = field_ty {
            type_path
                .path
                .segments
                .last()
                .map(|s| s.ident == "String")
                .unwrap_or(false)
        } else {
            false
        };

        if is_string {
            field_extractions.push(quote! {
                #field_name: __params.get(#path_key)
                    .ok_or_else(|| mik_sdk::typed::ParseError::missing(#path_key))?
                    .clone()
            });
        } else {
            field_extractions.push(quote! {
                #field_name: __params.get(#path_key)
                    .ok_or_else(|| mik_sdk::typed::ParseError::missing(#path_key))?
                    .parse()
                    .map_err(|_| mik_sdk::typed::ParseError::invalid_format(#path_key,
                        __params.get(#path_key).map(|s| s.as_str()).unwrap_or("")))?
            });
        }

        // Generate schema for this field (path params are always strings in OpenAPI)
        let escaped_path_key = escape_json_string(&path_key);
        schema_props.push(format!(r#""{}":{{"type":"string"}}"#, escaped_path_key));
        required_fields.push(format!(r#""{}""#, escaped_path_key));
    }

    let schema_props_str = schema_props.join(",");
    let required_str = required_fields.join(",");
    let schema_json = format!(
        r#"{{"type":"object","properties":{{{}}},"required":[{}]}}"#,
        schema_props_str, required_str
    );
    let name_str = name.to_string();

    let tokens = quote! {
        impl mik_sdk::typed::FromPath for #name {
            fn from_params(__params: &::std::collections::HashMap<String, String>) -> Result<Self, mik_sdk::typed::ParseError> {
                Ok(Self {
                    #(#field_extractions),*
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
        }
    };

    TokenStream::from(tokens)
}
