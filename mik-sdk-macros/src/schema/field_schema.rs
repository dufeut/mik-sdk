//! Route matching and handler wrapper code generation.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use super::type_schema::{InputSource, RouteDef, TypedInput};

// =============================================================================
// CODE GENERATION - ROUTE MATCHING
// =============================================================================

pub fn extract_param_names(pattern: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut chars = pattern.chars();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            if !name.is_empty() {
                params.push(name);
            }
        }
    }

    params
}

pub fn generate_pattern_matcher(pattern: &str) -> TokenStream2 {
    let params = extract_param_names(pattern);

    if params.is_empty() {
        quote! {
            (|| -> Option<::std::collections::HashMap<String, String>> {
                if __mik_path == #pattern {
                    Some(::std::collections::HashMap::new())
                } else {
                    None
                }
            })()
        }
    } else {
        let segments: Vec<&str> = pattern.split('/').collect();
        let segment_count = segments.len();

        let mut checks = Vec::new();
        let mut extractions = Vec::new();

        for (i, segment) in segments.iter().enumerate() {
            if segment.starts_with('{') && segment.ends_with('}') {
                // Parameter segment - URL decode the value
                let param_name = &segment[1..segment.len() - 1];
                extractions.push(quote! {
                    let __mik_raw_param = __mik_segments[#i];
                    // URL decode the path parameter. If decoding fails (malformed percent-encoding),
                    // fall back to the raw value. This is intentional: invalid encoding shouldn't
                    // crash the handler, and the raw value will either match the route or not.
                    let __mik_decoded_param = mik_sdk::url_decode(__mik_raw_param)
                        .unwrap_or_else(|_| __mik_raw_param.to_string());
                    __mik_params.insert(#param_name.to_string(), __mik_decoded_param);
                });
            } else if !segment.is_empty() {
                checks.push(quote! {
                    __mik_segments[#i] == #segment
                });
            } else if i > 0 {
                checks.push(quote! {
                    __mik_segments[#i].is_empty()
                });
            }
        }

        let all_checks = if checks.is_empty() {
            quote! { true }
        } else {
            quote! { #(#checks)&&* }
        };

        quote! {
            (|| -> Option<::std::collections::HashMap<String, String>> {
                let __mik_segments: Vec<&str> = __mik_path.split('/').collect();
                if __mik_segments.len() == #segment_count && #all_checks {
                    let mut __mik_params = ::std::collections::HashMap::new();
                    #(#extractions)*
                    Some(__mik_params)
                } else {
                    None
                }
            })()
        }
    }
}

// =============================================================================
// CODE GENERATION - HANDLER WRAPPERS
// =============================================================================

#[allow(clippy::too_many_lines)] // Complex input parsing with many type cases
pub fn generate_input_parsing(inputs: &[TypedInput]) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut parsing = Vec::new();
    let mut args = Vec::new();

    for (i, input) in inputs.iter().enumerate() {
        let var_name = format_ident!("__mik_input_{}", i);
        let type_name = &input.type_name;

        match input.source {
            InputSource::Path => {
                parsing.push(quote! {
                    let #var_name = match <#type_name as mik_sdk::typed::FromPath>::from_params(&__mik_params) {
                        Ok(v) => v,
                        Err(e) => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
            InputSource::Body => {
                parsing.push(quote! {
                    let #var_name = match __mik_req.body() {
                        Some(bytes) => {
                            match mik_sdk::json::try_parse(bytes) {
                                Some(json) => {
                                    match <#type_name as mik_sdk::typed::FromJson>::from_json(&json) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            return handler::Response {
                                                status: 400,
                                                headers: vec![
                                                    (
                                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                                    )
                                                ],
                                                body: Some(mik_sdk::json::obj()
                                                    .set("type", mik_sdk::json::str("about:blank"))
                                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                                    .set("status", mik_sdk::json::int(400))
                                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                                    .to_bytes()),
                                            };
                                        }
                                    }
                                }
                                None => {
                                    return handler::Response {
                                        status: 400,
                                        headers: vec![
                                            (
                                                mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                                mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                            )
                                        ],
                                        body: Some(mik_sdk::json::obj()
                                            .set("type", mik_sdk::json::str("about:blank"))
                                            .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                            .set("status", mik_sdk::json::int(400))
                                            .set("detail", mik_sdk::json::str("Invalid JSON body"))
                                            .to_bytes()),
                                    };
                                }
                            }
                        }
                        None => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str("Request body required"))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
            InputSource::Query => {
                parsing.push(quote! {
                    let __mik_query_params: Vec<(String, String)> = __mik_req.path()
                        .split_once('?')
                        .map(|(_, q)| {
                            q.split('&')
                                .filter_map(|pair| {
                                    let mut parts = pair.splitn(2, '=');
                                    Some((
                                        parts.next()?.to_string(),
                                        parts.next().unwrap_or("").to_string()
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let #var_name = match <#type_name as mik_sdk::typed::FromQuery>::from_query(&__mik_query_params) {
                        Ok(v) => v,
                        Err(e) => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
        }
    }

    (parsing, args)
}

pub fn generate_route_block(route: &RouteDef) -> TokenStream2 {
    let handler = &route.handler;
    let method_check = route.method.to_method_check();

    let pattern_checks: Vec<TokenStream2> = route
        .patterns
        .iter()
        .map(|pattern_str| {
            let matcher = generate_pattern_matcher(pattern_str);
            quote! {
                if let Some(__mik_params) = #matcher {
                    return Some(__mik_params);
                }
            }
        })
        .collect();

    let (input_parsing, input_args) = generate_input_parsing(&route.inputs);

    // Build handler call with typed inputs + &Request
    let handler_call = if input_args.is_empty() {
        quote! { #handler(&__mik_req) }
    } else {
        quote! { #handler(#(#input_args),*, &__mik_req) }
    };

    quote! {
        if __mik_method == #method_check {
            let __mik_try_match = || -> Option<::std::collections::HashMap<String, String>> {
                #(#pattern_checks)*
                None
            };

            if let Some(__mik_params) = __mik_try_match() {
                let __mik_req = mik_sdk::Request::new(
                    __mik_method.clone(),
                    __mik_raw.path.clone(),
                    __mik_raw.headers.clone(),
                    __mik_raw.body.clone(),
                    __mik_params.clone(),
                );

                #(#input_parsing)*

                return #handler_call;
            }
        }
    }
}
