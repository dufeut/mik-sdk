//! JSON value types and the json! macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, LitBool, LitFloat, LitInt, LitStr, Result, Token, braced, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token,
};

/// A JSON-like value for the macro.
pub enum JsonValue {
    Null,
    Bool(bool),
    Int(LitInt),
    Float(LitFloat),
    String(LitStr),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
    Expr(Expr),
    // Type hints for expressions
    StrHint(Expr),
    IntHint(Expr),
    FloatHint(Expr),
    BoolHint(Expr),
}

impl Parse for JsonValue {
    // JSON value parsing handles many literal types and nested structures
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(token::Brace) {
            // Object: { "key": value, ... }
            let content;
            braced!(content in input);
            let fields: Punctuated<KeyValue, Token![,]> =
                content.parse_terminated(KeyValue::parse, Token![,])
                    .map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid JSON object syntax.\n\
                                 Expected: {{ \"key\": value, ... }}\n\
                                 \n\
                                 Common mistakes:\n\
                                 - Keys must be string literals: {{ \"name\": \"Alice\" }} ✓ not {{ name: \"Alice\" }} ✗\n\
                                 - Use colon (:) between key and value: {{ \"x\": 1 }} ✓ not {{ \"x\" = 1 }} ✗\n\
                                 - Separate pairs with commas: {{ \"a\": 1, \"b\": 2 }} ✓\n\
                                 \n\
                                 Original error: {e}"
                            )
                        )
                    })?;
            Ok(Self::Object(
                fields.into_iter().map(|kv| (kv.key, kv.value)).collect(),
            ))
        } else if lookahead.peek(token::Bracket) {
            // Array: [value, ...]
            let content;
            bracketed!(content in input);
            let elements: Punctuated<Self, Token![,]> = content
                .parse_terminated(Self::parse, Token![,])
                .map_err(|e| {
                    syn::Error::new(
                        e.span(),
                        format!(
                            "Invalid JSON array syntax.\n\
                                 Expected: [value, value, ...]\n\
                                 \n\
                                 Common mistakes:\n\
                                 - Separate values with commas: [1, 2, 3] ✓ not [1 2 3] ✗\n\
                                 - Each element must be a valid JSON value\n\
                                 \n\
                                 Original error: {e}"
                        ),
                    )
                })?;
            Ok(Self::Array(elements.into_iter().collect()))
        } else if lookahead.peek(LitStr) {
            // String literal
            let lit: LitStr = input.parse()?;
            Ok(Self::String(lit))
        } else if lookahead.peek(LitInt) {
            // Integer literal
            let lit: LitInt = input.parse()?;
            Ok(Self::Int(lit))
        } else if lookahead.peek(LitFloat) {
            // Float literal
            let lit: LitFloat = input.parse()?;
            Ok(Self::Float(lit))
        } else if lookahead.peek(LitBool) {
            // Boolean literal
            let lit: LitBool = input.parse()?;
            Ok(Self::Bool(lit.value))
        } else if input.peek(syn::Ident) && input.peek2(token::Paren) {
            // Check for type hints: str(expr), int(expr), float(expr), bool(expr)
            let fork = input.fork();
            let ident: syn::Ident = fork.parse()?;
            match ident.to_string().as_str() {
                "str" => {
                    input.parse::<syn::Ident>()?; // consume "str"
                    let content;
                    syn::parenthesized!(content in input);
                    let expr: Expr = content.parse()
                        .map_err(|e| {
                            syn::Error::new(
                                e.span(),
                                format!(
                                    "Invalid expression inside str() type hint.\n\
                                     Expected: str(expression)\n\
                                     Example: str(user.name) or str(format!(\"Hello {{}}\", name))\n\
                                     \n\
                                     Original error: {e}"
                                )
                            )
                        })?;
                    Ok(Self::StrHint(expr))
                },
                "int" => {
                    input.parse::<syn::Ident>()?;
                    let content;
                    syn::parenthesized!(content in input);
                    let expr: Expr = content.parse().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid expression inside int() type hint.\n\
                                     Expected: int(expression)\n\
                                     Example: int(count) or int(items.len())\n\
                                     \n\
                                     Original error: {e}"
                            ),
                        )
                    })?;
                    Ok(Self::IntHint(expr))
                },
                "float" => {
                    input.parse::<syn::Ident>()?;
                    let content;
                    syn::parenthesized!(content in input);
                    let expr: Expr = content.parse().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid expression inside float() type hint.\n\
                                     Expected: float(expression)\n\
                                     Example: float(price) or float(3.14)\n\
                                     \n\
                                     Original error: {e}"
                            ),
                        )
                    })?;
                    Ok(Self::FloatHint(expr))
                },
                "bool" => {
                    input.parse::<syn::Ident>()?;
                    let content;
                    syn::parenthesized!(content in input);
                    let expr: Expr = content.parse().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid expression inside bool() type hint.\n\
                                     Expected: bool(expression)\n\
                                     Example: bool(is_active) or bool(count > 0)\n\
                                     \n\
                                     Original error: {e}"
                            ),
                        )
                    })?;
                    Ok(Self::BoolHint(expr))
                },
                _ => {
                    // Other ident followed by paren - parse as expression (function call)
                    let expr: Expr = input.parse()?;
                    Ok(Self::Expr(expr))
                },
            }
        } else if input.peek(syn::Ident) {
            // Check for null, true, false keywords
            let fork = input.fork();
            let ident: syn::Ident = fork.parse()?;
            match ident.to_string().as_str() {
                "null" => {
                    input.parse::<syn::Ident>()?; // consume it
                    Ok(Self::Null)
                },
                "true" => {
                    input.parse::<syn::Ident>()?;
                    Ok(Self::Bool(true))
                },
                "false" => {
                    input.parse::<syn::Ident>()?;
                    Ok(Self::Bool(false))
                },
                _ => {
                    // Parse as expression - syn handles comma boundaries
                    let expr: Expr = input.parse().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid JSON value or expression.\n\
                                     \n\
                                     Valid JSON values:\n\
                                     - String literal: \"hello\"\n\
                                     - Number: 42 or 3.14\n\
                                     - Boolean: true or false\n\
                                     - null\n\
                                     - Object: {{ \"key\": value }}\n\
                                     - Array: [1, 2, 3]\n\
                                     \n\
                                     For dynamic values, any expression implementing ToJson works:\n\
                                     - variable (String, i32, bool, Vec<T>, Option<T>, etc.)\n\
                                     \n\
                                     Or use explicit type hints:\n\
                                     - str(variable) for strings\n\
                                     - int(variable) for integers\n\
                                     - float(variable) for floats\n\
                                     - bool(variable) for booleans\n\
                                     \n\
                                     Original error: {e}"
                            ),
                        )
                    })?;
                    Ok(Self::Expr(expr))
                },
            }
        } else {
            // Try to parse as expression
            let expr: Expr = input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Invalid JSON value.\n\
                             \n\
                             Expected one of:\n\
                             - String literal: \"hello\"\n\
                             - Number: 42 or 3.14\n\
                             - Boolean: true or false\n\
                             - null\n\
                             - Object: {{ \"key\": value }}\n\
                             - Array: [1, 2, 3]\n\
                             - Expression implementing ToJson (String, i32, bool, Vec<T>, etc.)\n\
                             - Type hint: str(expr), int(expr), float(expr), bool(expr)\n\
                             \n\
                             Original error: {e}"
                    ),
                )
            })?;
            Ok(Self::Expr(expr))
        }
    }
}

struct KeyValue {
    key: String,
    value: JsonValue,
}

impl Parse for KeyValue {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: LitStr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Invalid object key.\n\
                         Expected: string literal\n\
                         \n\
                         Correct syntax:\n\
                         - {{ \"name\": \"Alice\" }} ✓\n\
                         - {{ \"age\": 30 }} ✓\n\
                         \n\
                         Common mistakes:\n\
                         - {{ name: \"Alice\" }} ✗ (missing quotes around key)\n\
                         - {{ 'name': \"Alice\" }} ✗ (use double quotes, not single)\n\
                         \n\
                         Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![:]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected colon (:) after object key.\n\
                         \n\
                         Correct syntax: {{ \"key\": value }}\n\
                         \n\
                         Common mistakes:\n\
                         - {{ \"key\" = value }} ✗ (use : not =)\n\
                         - {{ \"key\" value }} ✗ (missing colon)\n\
                         \n\
                         Original error: {e}"
                ),
            )
        })?;

        let value: JsonValue = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Invalid value in object.\n\
                         Expected a valid JSON value after the colon.\n\
                         \n\
                         Valid values:\n\
                         - {{ \"name\": \"Alice\" }} (string)\n\
                         - {{ \"age\": 30 }} (number)\n\
                         - {{ \"active\": true }} (boolean)\n\
                         - {{ \"data\": null }} (null)\n\
                         - {{ \"items\": [1, 2] }} (array)\n\
                         - {{ \"nested\": {{ \"key\": \"val\" }} }} (object)\n\
                         - {{ \"count\": int(items.len()) }} (type hint)\n\
                         \n\
                         Original error: {e}"
                ),
            )
        })?;

        Ok(Self {
            key: key.value(),
            value,
        })
    }
}

pub fn json_value_to_tokens(value: &JsonValue) -> TokenStream2 {
    match value {
        JsonValue::Null => quote! { json::null() },
        JsonValue::Bool(b) => quote! { json::bool(#b) },
        JsonValue::Int(i) => quote! { json::int(#i as i64) },
        JsonValue::Float(f) => quote! { json::float(#f) },
        JsonValue::String(s) => quote! { json::str(#s) },
        JsonValue::Array(elements) => {
            let pushes: Vec<TokenStream2> = elements
                .iter()
                .map(|e| {
                    let elem = json_value_to_tokens(e);
                    quote! { .push(#elem) }
                })
                .collect();
            quote! { json::arr()#(#pushes)* }
        },
        JsonValue::Object(fields) => {
            let sets: Vec<TokenStream2> = fields
                .iter()
                .map(|(k, v)| {
                    let val = json_value_to_tokens(v);
                    quote! { .set(#k, #val) }
                })
                .collect();
            quote! { json::obj()#(#sets)* }
        },
        JsonValue::Expr(e) => {
            // Use ToJson trait for automatic type inference
            quote! { json::ToJson::to_json(&(#e)) }
        },
        // Type hints - wrap expressions with appropriate json:: calls
        JsonValue::StrHint(e) => quote! { json::str(&#e) },
        JsonValue::IntHint(e) => quote! { json::int(#e as i64) },
        JsonValue::FloatHint(e) => quote! { json::float(#e as f64) },
        JsonValue::BoolHint(e) => quote! { json::bool(#e) },
    }
}

/// Create a JSON value with clean syntax.
///
/// # Type Inference
///
/// Any expression implementing [`ToJson`] can be used directly:
/// - `String`, `&str` - becomes JSON string
/// - `i8..i64`, `u8..u64`, `usize` - becomes JSON integer
/// - `f32`, `f64` - becomes JSON float
/// - `bool` - becomes JSON boolean
/// - `Option<T>` - becomes value or null
/// - `Vec<T>`, `&[T]` - becomes JSON array
///
/// # Type Hints (Optional)
///
/// Explicit type hints are still available for when inference doesn't work:
/// - `str(expr)` - Convert to JSON string
/// - `int(expr)` - Convert to JSON integer
/// - `float(expr)` - Convert to JSON float
/// - `bool(expr)` - Convert to JSON boolean
///
/// # Examples
///
/// ```ignore
/// use mik_sdk::prelude::*;
///
/// // Literals work directly
/// let obj = json!({
///     "name": "Alice",
///     "age": 30,
///     "active": true
/// });
///
/// let arr = json!(["one", "two", "three"]);
///
/// // Variables with ToJson types work directly (NEW!)
/// let name = "Bob".to_string();
/// let age: i32 = 25;
/// let tags = vec!["admin", "user"];
///
/// let dynamic = json!({
///     "name": name,        // String -> JSON string
///     "age": age,          // i32 -> JSON integer
///     "tags": tags,        // Vec<&str> -> JSON array
///     "score": 3.14_f64    // f64 -> JSON float
/// });
///
/// // Type hints still work for explicit control
/// let explicit = json!({
///     "count": int(items.len())  // usize needs hint if .len() is ambiguous
/// });
/// ```
pub fn json_impl(input: TokenStream) -> TokenStream {
    let value = parse_macro_input!(input as JsonValue);
    let tokens = json_value_to_tokens(&value);
    TokenStream::from(tokens)
}
