//! Derive macros for typed inputs: Type, Query, Path.
//!
//! These generate implementations for FromJson, FromQuery, FromPath traits,
//! along with OpenAPI schema generation and optional validation.

mod path_derive;
mod query_derive;
mod type_derive;

use std::fmt::Write;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, Lit, Type};

use crate::errors::did_you_mean;

/// Valid #[field(...)] attributes.
const VALID_FIELD_ATTRS: &[&str] = &[
    "min", "max", "default", "format", "pattern", "rename", "docs",
];

// Re-export the public entry points
pub use path_derive::derive_path_impl;
pub use query_derive::derive_query_impl;
pub use type_derive::derive_type_impl;

// ============================================================================
// JSON STRING ESCAPING
// ============================================================================

/// Escape a string for use in JSON output.
/// Handles all JSON control characters per RFC 8259.
pub fn escape_json_string(s: &str) -> String {
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
                let _ = write!(result, "\\u{:04x}", c as u32);
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
pub struct FieldAttrs {
    pub(crate) min: Option<i64>,
    pub(crate) max: Option<i64>,
    pub(crate) format: Option<String>,
    pub(crate) pattern: Option<String>,
    pub(crate) default: Option<String>,
    pub(crate) rename: Option<String>,
    pub(crate) docs: Option<String>,
}

#[allow(clippy::too_many_lines)]
pub fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut result = FieldAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("min") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Int(lit) => {
                        result.min = lit.base10_parse().ok();
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "min needs a number!\n\
                             \n\
                             ✅ Correct: #[field(min = 1)]\n\
                             ❌ Wrong:   #[field(min = \"1\")]",
                        ));
                    },
                }
            } else if meta.path.is_ident("max") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Int(lit) => {
                        result.max = lit.base10_parse().ok();
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "max needs a number!\n\
                             \n\
                             ✅ Correct: #[field(max = 100)]\n\
                             ❌ Wrong:   #[field(max = \"100\")]",
                        ));
                    },
                }
            } else if meta.path.is_ident("format") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Str(lit) => {
                        result.format = Some(lit.value());
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "format needs a string!\n\
                             \n\
                             ✅ Correct: #[field(format = \"email\")]\n\
                             ❌ Wrong:   #[field(format = email)]",
                        ));
                    },
                }
            } else if meta.path.is_ident("pattern") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Str(lit) => {
                        result.pattern = Some(lit.value());
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "pattern needs a string (regex)!\n\
                             \n\
                             ✅ Correct: #[field(pattern = r\"^[a-z]+$\")]\n\
                             ❌ Wrong:   #[field(pattern = ^[a-z]+$)]",
                        ));
                    },
                }
            } else if meta.path.is_ident("default") {
                let value: Expr = meta.value()?.parse()?;
                result.default = Some(quote!(#value).to_string());
            } else if meta.path.is_ident("rename") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Str(lit) => {
                        result.rename = Some(lit.value());
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "rename needs a string!\n\
                             \n\
                             ✅ Correct: #[field(rename = \"userName\")]\n\
                             ❌ Wrong:   #[field(rename = userName)]",
                        ));
                    },
                }
            } else if meta.path.is_ident("docs") {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Str(lit) => {
                        result.docs = Some(lit.value());
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &value,
                            "docs needs a string!\n\
                             \n\
                             ✅ Correct: #[field(docs = \"The user's email\")]",
                        ));
                    },
                }
            } else {
                let path = &meta.path;
                let attr_name = quote!(#path).to_string();
                let suggestion = did_you_mean(&attr_name, VALID_FIELD_ATTRS);
                return Err(syn::Error::new_spanned(
                    path,
                    format!(
                        "Unknown field attribute '{attr_name}'.{suggestion}\n\
                         \n\
                         ✅ Valid attributes:\n\
                         #[field(min = 1)]           // minimum value/length\n\
                         #[field(max = 100)]         // maximum value/length\n\
                         #[field(default = 10)]      // default value\n\
                         #[field(format = \"email\")] // format hint (OpenAPI)\n\
                         #[field(pattern = \"...\")]  // regex pattern (OpenAPI)\n\
                         #[field(rename = \"...\")]   // JSON key name\n\
                         #[field(docs = \"...\")]     // description"
                    ),
                ));
            }
            Ok(())
        })?;
    }

    Ok(result)
}

// ============================================================================
// TYPE HELPERS (delegating to centralized type_registry)
// ============================================================================

// Re-export from type_registry for backward compatibility
pub use crate::type_registry::{get_inner_type, is_option_type};

/// Get OpenAPI schema for a type.
pub fn type_to_openapi(ty: &Type) -> String {
    crate::type_registry::get_openapi_schema(ty)
}

/// Get JSON getter TokenStream for a type.
pub fn rust_type_to_json_getter(ty: &Type) -> Option<TokenStream2> {
    crate::type_registry::get_json_getter(ty)
}

/// Get a human-readable type name for error messages.
pub fn rust_type_to_name(ty: &Type) -> &'static str {
    crate::type_registry::get_type_name(ty)
}

// ============================================================================
// STRUCT FIELD EXTRACTION HELPER
// ============================================================================

/// Context for derive macro error messages (Query and Path only)
///
/// Note: Type derive handles its own error messages since it supports both structs and enums.
#[derive(Clone, Copy)]
pub enum DeriveContext {
    Query,
    Path,
}

impl DeriveContext {
    const fn name(self) -> &'static str {
        match self {
            Self::Query => "Query",
            Self::Path => "Path",
        }
    }

    const fn example(self) -> &'static str {
        match self {
            Self::Query => "struct MyQuery { page: u32, limit: u32 }",
            Self::Path => "struct UserPath { org_id: String, id: String }",
        }
    }

    const fn purpose(self) -> &'static str {
        match self {
            Self::Query => "for query parameters",
            Self::Path => "for URL path parameters",
        }
    }
}

/// Extract named fields from a DeriveInput, returning an error TokenStream if invalid.
pub fn extract_named_fields(
    input: &DeriveInput,
    ctx: DeriveContext,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            Fields::Unnamed(_) => Err(syn::Error::new_spanned(
                input,
                format!(
                    "Oops! #[derive({})] needs named fields, not tuple fields.\n\
                     \n\
                     ❌ What you have (tuple struct):\n\
                       struct MyStruct(String, i32);\n\
                     \n\
                     ✅ What you need (named fields):\n\
                       #[derive({})]\n\
                       {}\n\
                     \n\
                     Named fields have names like 'id', 'name', 'email' - they make\n\
                     your code easier to read and work with JSON/query params.",
                    ctx.name(),
                    ctx.name(),
                    ctx.example()
                ),
            )
            .to_compile_error()
            .into()),
            Fields::Unit => Err(syn::Error::new_spanned(
                input,
                format!(
                    "Oops! #[derive({})] needs a struct with fields.\n\
                     \n\
                     ❌ What you have (empty struct):\n\
                       struct MyStruct;\n\
                     \n\
                     ✅ What you need:\n\
                       #[derive({})]\n\
                       {}\n\
                     \n\
                     Add some fields to hold your data!",
                    ctx.name(),
                    ctx.name(),
                    ctx.example()
                ),
            )
            .to_compile_error()
            .into()),
        },
        Data::Enum(_) => Err(syn::Error::new_spanned(
            input,
            format!(
                "Oops! #[derive({})] only works on structs, not enums.\n\
                 \n\
                 ❌ What you have:\n\
                   enum MyEnum {{ A, B, C }}\n\
                 \n\
                 ✅ What you need:\n\
                   #[derive({})]\n\
                   {}\n\
                 \n\
                 {} needs a struct {}.",
                ctx.name(),
                ctx.name(),
                ctx.example(),
                ctx.name(),
                ctx.purpose()
            ),
        )
        .to_compile_error()
        .into()),
        Data::Union(_) => Err(syn::Error::new_spanned(
            input,
            format!(
                "Oops! #[derive({})] only works on structs, not unions.\n\
                 \n\
                 ✅ What you need:\n\
                   #[derive({})]\n\
                   {}\n\
                 \n\
                 {} needs a struct {}.",
                ctx.name(),
                ctx.name(),
                ctx.example(),
                ctx.name(),
                ctx.purpose()
            ),
        )
        .to_compile_error()
        .into()),
    }
}
