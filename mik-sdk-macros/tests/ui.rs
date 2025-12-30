//! UI tests for proc macro error messages.
//!
//! These tests verify that our proc macros emit clear, helpful error messages
//! when users make mistakes. Run with `cargo test --test ui`.
//!
//! To update expected output after intentional changes:
//! ```
//! TRYBUILD=overwrite cargo test --test ui
//! ```

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // Passing test cases - basic valid usage
    t.pass("tests/ui/pass/*.rs");

    // Derive macro errors
    t.compile_fail("tests/ui/derive/*.rs");

    // Routes macro errors
    t.compile_fail("tests/ui/routes/*.rs");

    // JSON macro errors
    t.compile_fail("tests/ui/json/*.rs");

    // Response macro errors
    t.compile_fail("tests/ui/response/*.rs");

    // HTTP client macro errors
    t.compile_fail("tests/ui/fetch/*.rs");

    // DX macro errors (guard!, ensure!)
    t.compile_fail("tests/ui/dx/*.rs");
}
