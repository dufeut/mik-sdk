//! #[derive(Path)] implementation for URL path parameter types.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type, parse_macro_input};

use super::{DeriveContext, escape_json_string, extract_named_fields, parse_field_attrs};

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
                .is_some_and(|s| s.ident == "String")
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
        schema_props.push(format!(r#""{escaped_path_key}":{{"type":"string"}}"#));
        required_fields.push(format!(r#""{escaped_path_key}""#));
    }

    let schema_props_str = schema_props.join(",");
    let required_str = required_fields.join(",");
    let schema_json = format!(
        r#"{{"type":"object","properties":{{{schema_props_str}}},"required":[{required_str}]}}"#
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
