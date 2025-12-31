//! Macro expansion snapshot tests.
//!
//! These tests capture the expanded output of macros and compare against
//! saved snapshots. This catches regressions in macro expansion.
//!
//! To update snapshots after intentional changes:
//! ```bash
//! MACROTEST=overwrite cargo test --test expand
//! ```

#[test]
fn expand_macros() {
    macrotest::expand("tests/expand/*.rs");
}
