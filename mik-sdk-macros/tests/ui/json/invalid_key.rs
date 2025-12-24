use mik_sdk_macros::json;

// Error: Invalid JSON key - expected string literal
fn main() {
    let _v = json!({ 123: "value" });
}
