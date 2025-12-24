//! UI tests for SQL proc macro error messages.
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

    // SQL macro errors
    t.compile_fail("tests/ui/sql/*.rs");
}
