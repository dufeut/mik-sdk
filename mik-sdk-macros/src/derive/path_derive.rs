//! #[derive(Path)] implementation for URL path parameter types.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type, parse_macro_input};

use super::{DeriveContext, escape_json_string, extract_named_fields, parse_field_attrs};
use crate::openapi::utoipa::{
    FieldDef, object_schema, ref_or_schema_to_json, rust_type_to_schema, schema_to_json,
};

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
    let mut field_defs = Vec::new(); // utoipa FieldDef for object schema
    let mut path_params = Vec::new(); // OpenAPI path parameter objects

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

        // Build utoipa schema for this field
        let type_str = quote::quote!(#field_ty).to_string().replace(' ', "");
        let field_schema = rust_type_to_schema(&type_str);

        // Add field definition for object schema (all path params are required)
        field_defs.push(FieldDef {
            name: path_key.clone(),
            schema: field_schema.clone(),
            required: true,
        });

        // Build OpenAPI path parameter object using utoipa schema
        let schema_json = ref_or_schema_to_json(&field_schema);
        let escaped_path_key = escape_json_string(&path_key);
        path_params.push(format!(
            r#"{{"name":"{escaped_path_key}","in":"path","required":true,"schema":{schema_json}}}"#
        ));
    }

    // Build the object schema using utoipa's object_schema helper
    let schema = object_schema(field_defs);
    let schema_json = schema_to_json(&schema);
    let name_str = name.to_string();

    // Build OpenAPI path parameters array
    let path_params_json = format!("[{}]", path_params.join(","));

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

            fn openapi_path_params() -> &'static str {
                #path_params_json
            }
        }
    };

    TokenStream::from(tokens)
}
