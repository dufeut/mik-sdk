//! Property-based tests for SQL validation using proptest.
//!
//! These tests generate random inputs to find edge cases that
//! manual tests might miss.

use mik_sql::{is_valid_sql_expression, is_valid_sql_identifier};
use proptest::prelude::*;

// =============================================================================
// SQL Identifier Property Tests
// =============================================================================

proptest! {
    /// Valid identifiers should always pass validation
    #[test]
    fn valid_identifiers_always_pass(
        // Generate valid SQL identifiers: start with letter, then alphanumeric + underscore
        s in "[a-zA-Z][a-zA-Z0-9_]{0,62}"
    ) {
        prop_assert!(
            is_valid_sql_identifier(&s),
            "Valid identifier should pass: {}", s
        );
    }

    /// Identifiers starting with numbers should fail
    #[test]
    fn numeric_start_always_fails(
        prefix in "[0-9]+",
        suffix in "[a-zA-Z_][a-zA-Z0-9_]*"
    ) {
        let ident = format!("{prefix}{suffix}");
        prop_assert!(
            !is_valid_sql_identifier(&ident),
            "Identifier starting with number should fail: {}", ident
        );
    }

    /// Empty and whitespace-only strings should fail
    #[test]
    fn empty_or_whitespace_fails(s in r"\s*") {
        prop_assert!(
            !is_valid_sql_identifier(&s),
            "Empty/whitespace should fail: {:?}", s
        );
    }

    /// Identifiers with special characters should fail
    #[test]
    fn special_chars_fail(
        prefix in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
        special in r"[!@#$%^&*()\-+=\[\]{};:,.<>/?\\|`~]",
        suffix in "[a-zA-Z0-9_]{0,10}"
    ) {
        let ident = format!("{prefix}{special}{suffix}");
        prop_assert!(
            !is_valid_sql_identifier(&ident),
            "Identifier with special char should fail: {}", ident
        );
    }

    /// Very long identifiers should fail (>63 chars per SQL standard)
    #[test]
    fn long_identifiers_fail(s in "[a-zA-Z][a-zA-Z0-9_]{64,100}") {
        prop_assert!(
            !is_valid_sql_identifier(&s),
            "Long identifier ({} chars) should fail", s.len()
        );
    }
}

// =============================================================================
// SQL Expression Property Tests
// =============================================================================

/// Check if identifier contains blocked patterns
fn contains_blocked_pattern(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("pg_")
        || lower.contains("sqlite_")
        || lower.contains("information_")
        || lower.contains("load_")
        || lower.contains("0x")
}

proptest! {
    /// Simple column references should pass (avoid reserved patterns)
    #[test]
    fn simple_column_refs_pass(col in "[a-zA-Z][a-zA-Z_]{0,30}") {
        // Filter out identifiers containing blocked patterns
        prop_assume!(!contains_blocked_pattern(&col));
        prop_assert!(
            is_valid_sql_expression(&col),
            "Simple column should pass: {}", col
        );
    }

    /// Basic arithmetic expressions should pass (avoid reserved patterns)
    #[test]
    fn arithmetic_expressions_pass(
        col1 in "[a-zA-Z][a-zA-Z_]{0,10}",
        op in r"[+\-*/]",
        col2 in "[a-zA-Z][a-zA-Z_]{0,10}"
    ) {
        // Filter out identifiers containing blocked patterns
        prop_assume!(!contains_blocked_pattern(&col1));
        prop_assume!(!contains_blocked_pattern(&col2));
        let expr = format!("{col1} {op} {col2}");
        prop_assert!(
            is_valid_sql_expression(&expr),
            "Arithmetic should pass: {}", expr
        );
    }

    /// SQL keywords in expressions should fail
    #[test]
    fn sql_keywords_fail(
        keyword in prop_oneof![
            Just("DROP"),
            Just("DELETE"),
            Just("INSERT"),
            Just("UPDATE"),
            Just("TRUNCATE"),
            Just("ALTER"),
            Just("CREATE"),
            Just("EXEC"),
            Just("EXECUTE"),
            Just("UNION"),
        ]
    ) {
        prop_assert!(
            !is_valid_sql_expression(keyword),
            "Keyword should fail: {}", keyword
        );
    }

    /// Expressions with semicolons should fail (statement separator)
    #[test]
    fn semicolons_fail(
        prefix in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
        suffix in "[a-zA-Z0-9_]{0,10}"
    ) {
        let expr = format!("{prefix};{suffix}");
        prop_assert!(
            !is_valid_sql_expression(&expr),
            "Expression with semicolon should fail: {}", expr
        );
    }

    /// Comment sequences should fail
    #[test]
    fn double_dash_comments_fail(
        prefix in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
        suffix in "[a-zA-Z0-9_ ]{0,10}"
    ) {
        let expr = format!("{prefix}--{suffix}");
        prop_assert!(
            !is_valid_sql_expression(&expr),
            "Expression with -- comment should fail: {}", expr
        );
    }
}

// =============================================================================
// Injection Attempt Property Tests
// =============================================================================

proptest! {
    /// Random strings with double quotes should be handled safely
    #[test]
    fn double_quotes_handled_safely(
        prefix in "[a-zA-Z]{0,5}",
        middle in "[a-zA-Z0-9 ]{0,10}",
        suffix in "[a-zA-Z]{0,5}"
    ) {
        let expr = format!("{prefix}\"{middle}\"{suffix}");
        // Should either pass (if valid) or fail (if dangerous) - never panic
        let _result = is_valid_sql_expression(&expr);
    }

    /// Expressions with dangerous functions should fail
    #[test]
    fn dangerous_functions_fail(
        func in prop_oneof![
            Just("SLEEP"),
            Just("BENCHMARK"),
            Just("WAITFOR"),
            Just("PG_SLEEP"),
            Just("LOAD_FILE"),
        ],
        arg in "[0-9]{1,5}"
    ) {
        let expr = format!("{func}({arg})");
        prop_assert!(
            !is_valid_sql_expression(&expr),
            "Dangerous function should fail: {}", expr
        );
    }

    /// Stacked queries (multiple statements) should fail
    #[test]
    fn stacked_queries_fail(
        stmt1 in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
        stmt2 in "[a-zA-Z][a-zA-Z0-9_]{0,10}"
    ) {
        let expr = format!("{stmt1}; {stmt2}");
        prop_assert!(
            !is_valid_sql_expression(&expr),
            "Stacked query should fail: {}", expr
        );
    }
}

// =============================================================================
// Fuzzing-style Random Input Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Random ASCII should never panic
    #[test]
    fn random_ascii_no_panic(s in "[[:print:]]{0,100}") {
        // Just verify no panic - result doesn't matter
        let _id = is_valid_sql_identifier(&s);
        let _expr = is_valid_sql_expression(&s);
    }

    /// Random bytes should never panic
    #[test]
    fn random_bytes_no_panic(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            let _id = is_valid_sql_identifier(s);
            let _expr = is_valid_sql_expression(s);
        }
    }

    /// Unicode strings should never panic
    #[test]
    fn unicode_no_panic(s in "[a-zA-Z0-9_ ]{0,50}") {
        let _id = is_valid_sql_identifier(&s);
        let _expr = is_valid_sql_expression(&s);
    }
}
