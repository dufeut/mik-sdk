use mik_sdk_macros::json;

// Error: Expected ':' after key
fn main() {
    let _v = json!({ "key" "value" });
}
