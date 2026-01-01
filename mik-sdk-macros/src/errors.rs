//! Error helper utilities for consistent, informative compile-time errors.
//!
//! This module provides utilities for building user-friendly error messages
//! in proc-macros with examples and context.
//!
//! These helpers are available for use in improving error messages throughout
//! the macro implementations.

#![allow(dead_code)] // Helpers provided for future use

use proc_macro2::Span;
use syn::Error;

// =============================================================================
// FUZZY MATCHING ("DID YOU MEAN?")
// =============================================================================

/// Calculate the Levenshtein edit distance between two strings.
/// Used for "did you mean?" suggestions.
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
/// Returns `Some(suggestion)` if a close match is found (within threshold).
pub fn find_similar<'a>(input: &str, options: &[&'a str]) -> Option<&'a str> {
    if options.is_empty() {
        return None;
    }

    let input_len = input.len();
    // Threshold: allow ~40% of chars to be wrong, minimum 2, maximum 4
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

/// Build a formatted error with examples.
///
/// # Examples
///
/// ```ignore
/// use crate::errors::parse_error;
///
/// return Err(parse_error(
///     span,
///     "Invalid field type",
///     &["str(expr)", "int(expr)", "float(expr)", "bool(expr)"]
/// ));
/// ```
pub fn parse_error(span: Span, message: &str, examples: &[&str]) -> Error {
    let examples_str = examples
        .iter()
        .map(|e| format!("  {e}"))
        .collect::<Vec<_>>()
        .join("\n");

    Error::new(span, format!("{message}\n\nExamples:\n{examples_str}"))
}

/// Build an error for an unknown identifier with valid options.
/// Includes "did you mean?" suggestion if a similar option exists.
///
/// # Examples
///
/// ```ignore
/// use crate::errors::unknown_error;
///
/// // User typed "PSOT" instead of "POST"
/// return Err(unknown_error(
///     span,
///     "HTTP method",
///     "PSOT",
///     &["GET", "POST", "PUT", "DELETE"]
/// ));
/// // Error: Unknown HTTP method 'PSOT'.
/// //
/// // Did you mean 'POST'?
/// //
/// // Valid options: GET, POST, PUT, DELETE
/// ```
pub fn unknown_error(span: Span, kind: &str, got: &str, valid: &[&str]) -> Error {
    let suggestion = did_you_mean(got, valid);
    let valid_str = valid.join(", ");
    Error::new(
        span,
        format!("Unknown {kind} '{got}'.{suggestion}\n\nValid options: {valid_str}"),
    )
}

/// Build an error for a missing required field.
///
/// # Examples
///
/// ```ignore
/// use crate::errors::missing_field_error;
///
/// return Err(missing_field_error(
///     span,
///     "status",
///     "error! { status: 404, title: \"Not Found\" }"
/// ));
/// ```
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

/// Build an error for an invalid field attribute value.
///
/// # Examples
///
/// ```ignore
/// use crate::errors::invalid_attr;
///
/// return Err(invalid_attr(
///     span,
///     "min",
///     "a number",
///     "#[field(min = 1)]"
/// ));
/// ```
pub fn invalid_attr(span: Span, attr: &str, expected: &str, example: &str) -> Error {
    Error::new(
        span,
        format!("'{attr}' expects {expected}.\n\n\u{2705} Correct: {example}",),
    )
}

/// Build an error for expected syntax.
///
/// # Examples
///
/// ```ignore
/// use crate::errors::expected_syntax;
///
/// return Err(expected_syntax(
///     span,
///     "=>",
///     "after route path",
///     "GET \"/users\" => list_users"
/// ));
/// ```
pub fn expected_syntax(span: Span, expected: &str, context: &str, example: &str) -> Error {
    Error::new(
        span,
        format!("Expected {expected} {context}.\n\nExample: {example}"),
    )
}

/// Build an error for type mismatch.
pub fn type_mismatch(span: Span, field: &str, expected: &str, got: &str) -> Error {
    Error::new(
        span,
        format!("Type mismatch for '{field}'.\n\nExpected: {expected}\nGot: {got}"),
    )
}

/// Build an error for unsupported construct.
pub fn unsupported(span: Span, what: &str, suggestion: &str) -> Error {
    Error::new(span, format!("{what}\n\n\u{2705} Try: {suggestion}"))
}

/// Extension trait to add context to errors.
pub trait ErrorContext {
    /// Wrap an error with additional context.
    fn with_context(self, context: &str) -> Error;
}

impl ErrorContext for Error {
    fn with_context(self, context: &str) -> Self {
        Self::new(self.span(), format!("{context}\n\nCaused by: {self}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Levenshtein Distance Tests
    // =========================================================================

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_case_insensitive() {
        assert_eq!(levenshtein_distance("Hello", "hello"), 0);
        assert_eq!(levenshtein_distance("POST", "post"), 0);
    }

    #[test]
    fn test_levenshtein_one_char_diff() {
        assert_eq!(levenshtein_distance("post", "pust"), 1); // substitution
        assert_eq!(levenshtein_distance("post", "posts"), 1); // insertion
        assert_eq!(levenshtein_distance("posts", "post"), 1); // deletion
    }

    #[test]
    fn test_levenshtein_transposition() {
        // "psot" vs "post" - 2 operations (not Damerau-Levenshtein)
        assert_eq!(levenshtein_distance("psot", "post"), 2);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    // =========================================================================
    // Find Similar Tests
    // =========================================================================

    #[test]
    fn test_find_similar_exact_match_returns_none() {
        // Exact match has distance 0, filtered out
        assert_eq!(find_similar("GET", &["GET", "POST"]), None);
    }

    #[test]
    fn test_find_similar_close_typo() {
        assert_eq!(find_similar("PSOT", &["GET", "POST", "PUT"]), Some("POST"));
        assert_eq!(find_similar("GTE", &["GET", "POST", "PUT"]), Some("GET"));
        assert_eq!(
            find_similar("DELTE", &["GET", "POST", "DELETE"]),
            Some("DELETE")
        );
    }

    #[test]
    fn test_find_similar_case_insensitive() {
        assert_eq!(find_similar("psot", &["GET", "POST", "PUT"]), Some("POST"));
    }

    #[test]
    fn test_find_similar_no_match_too_different() {
        assert_eq!(find_similar("xyz", &["GET", "POST", "PUT"]), None);
    }

    #[test]
    fn test_find_similar_operator_typos() {
        let operators = &["$eq", "$ne", "$gt", "$gte", "$lt", "$lte", "$in"];
        assert_eq!(find_similar("$equ", operators), Some("$eq"));
        assert_eq!(find_similar("$gtr", operators), Some("$gt"));
        assert_eq!(find_similar("$between", operators), None); // too different
        assert_eq!(find_similar("$inn", operators), Some("$in"));
    }

    #[test]
    fn test_find_similar_empty_options() {
        assert_eq!(find_similar("test", &[]), None);
    }

    // =========================================================================
    // Error Helper Tests
    // =========================================================================

    #[test]
    fn test_parse_error_formats_correctly() {
        let err = parse_error(Span::call_site(), "Invalid value", &["str(x)", "int(y)"]);
        let msg = err.to_string();
        assert!(msg.contains("Invalid value"));
        assert!(msg.contains("str(x)"));
        assert!(msg.contains("int(y)"));
    }

    #[test]
    fn test_unknown_error_formats_correctly() {
        let err = unknown_error(Span::call_site(), "operator", "$bad", &["$eq", "$ne"]);
        let msg = err.to_string();
        assert!(msg.contains("Unknown operator"));
        assert!(msg.contains("$bad"));
        assert!(msg.contains("$eq, $ne"));
    }

    #[test]
    fn test_unknown_error_with_suggestion() {
        let err = unknown_error(
            Span::call_site(),
            "HTTP method",
            "PSOT",
            &["GET", "POST", "PUT", "DELETE"],
        );
        let msg = err.to_string();
        assert!(msg.contains("Unknown HTTP method"));
        assert!(msg.contains("PSOT"));
        assert!(msg.contains("Did you mean 'POST'?"));
    }

    #[test]
    fn test_unknown_error_no_suggestion_when_too_different() {
        let err = unknown_error(
            Span::call_site(),
            "HTTP method",
            "FOOBAR",
            &["GET", "POST", "PUT", "DELETE"],
        );
        let msg = err.to_string();
        assert!(!msg.contains("Did you mean"));
    }

    #[test]
    fn test_did_you_mean_helper() {
        assert_eq!(
            did_you_mean("PSOT", &["GET", "POST"]),
            "\n\nDid you mean 'POST'?"
        );
        assert_eq!(did_you_mean("xyz", &["GET", "POST"]), "");
    }
}
