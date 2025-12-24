use mik_sdk_macros::guard;

// Error: Missing message string
fn main() {
    let name = "test";
    guard!(!name.is_empty(), 400);
}
