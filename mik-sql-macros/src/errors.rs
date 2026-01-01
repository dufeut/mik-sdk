//! Error helper utilities for consistent, informative compile-time errors.
//!
//! This module provides utilities for building user-friendly error messages
//! in proc-macros with examples and context.

#![allow(dead_code)] // Helpers provided for future use

use proc_macro2::Span;
use syn::Error;

// =============================================================================
// FUZZY MATCHING ("DID YOU MEAN?")
// =============================================================================

/// Calculate the Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    if a_chars.is_empty() {
        return b_chars.len();
    }
    if b_chars.is_empty() {
        return a_chars.len();
    }

    let mut prev_row: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr_row = vec![0; b_chars.len() + 1];

    for (i, a_char) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = usize::from(a_char != b_char);
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_chars.len()]
}

/// Find the most similar option to the given input.
pub fn find_similar<'a>(input: &str, options: &[&'a str]) -> Option<&'a str> {
    if options.is_empty() {
        return None;
    }

    let input_len = input.len();
    let threshold = (input_len / 2).clamp(2, 4);

    options
        .iter()
        .map(|opt| (*opt, levenshtein_distance(input, opt)))
        .filter(|(_, dist)| *dist <= threshold && *dist > 0)
        .min_by_key(|(_, dist)| *dist)
        .map(|(opt, _)| opt)
}

/// Format a "did you mean?" suggestion if a similar option exists.
#[allow(clippy::option_if_let_else)] // match is more readable here
pub fn did_you_mean(input: &str, options: &[&str]) -> String {
    match find_similar(input, options) {
        Some(suggestion) => format!("\n\nDid you mean '{suggestion}'?"),
        None => String::new(),
    }
}

// =============================================================================
// ERROR HELPERS
// =============================================================================

/// Build an error for an unknown identifier with valid options.
pub fn unknown_error(span: Span, kind: &str, got: &str, valid: &[&str]) -> Error {
    let suggestion = did_you_mean(got, valid);
    let valid_str = valid.join(", ");
    Error::new(
        span,
        format!("Unknown {kind} '{got}'.{suggestion}\n\nValid options: {valid_str}"),
    )
}

/// Build an error for a missing required field.
pub fn missing_field_error(span: Span, field: &str, example: &str) -> Error {
    Error::new(
        span,
        format!("Missing required field '{field}'.\n\nExample:\n  {example}"),
    )
}

/// Build an error for a duplicate field.
pub fn duplicate_field_error(span: Span, field: &str) -> Error {
    Error::new(
        span,
        format!("Duplicate '{field}' field. Each field can only appear once."),
    )
}

/// Build an error for expected syntax.
pub fn expected_syntax(span: Span, expected: &str, context: &str, example: &str) -> Error {
    Error::new(
        span,
        format!("Expected {expected} {context}.\n\nExample: {example}"),
    )
}

/// Build an error for an empty block that requires content.
pub fn empty_block_error(span: Span, block_type: &str, example: &str) -> Error {
    Error::new(
        span,
        format!("Empty {block_type} block. At least one item required.\n\nExample:\n  {example}"),
    )
}

/// Valid SQL filter operators.
const VALID_OPERATORS: &[&str] = &[
    "eq",
    "ne",
    "gt",
    "gte",
    "lt",
    "lte",
    "in",
    "nin",
    "like",
    "ilike",
    "starts_with",
    "startsWith",
    "ends_with",
    "endsWith",
    "contains",
    "between",
    "and",
    "or",
    "not",
    "regex",
];

/// Build an error for invalid operator.
pub fn invalid_operator(span: Span, op: &str) -> Error {
    let suggestion = did_you_mean(op, VALID_OPERATORS);
    Error::new(
        span,
        format!(
            "Unknown operator '${op}'.{suggestion}\n\n\
             Valid operators:\n\
             \u{2022} Comparison: $eq, $ne, $gt, $gte, $lt, $lte\n\
             \u{2022} Collection: $in, $nin\n\
             \u{2022} String: $like, $ilike, $starts_with, $ends_with, $contains\n\
             \u{2022} Range: $between\n\
             \u{2022} Logical: $and, $or, $not"
        ),
    )
}
