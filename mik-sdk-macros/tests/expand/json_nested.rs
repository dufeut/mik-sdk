// Test json! macro with nested structures
use mik_sdk::json;

fn main() {
    let value = json!({
        "user": {
            "name": "Bob",
            "address": {
                "city": "NYC"
            }
        },
        "tags": ["a", "b", "c"]
    });
    let _ = value;
}
