use mik_sdk_macros::guard;

// Error: Missing comma after condition
fn main() {
    let name = "test";
    guard!(!name.is_empty() 400, "Name required");
}
