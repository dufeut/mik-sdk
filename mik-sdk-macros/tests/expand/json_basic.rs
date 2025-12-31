// Test json! macro expansion
use mik_sdk::json;

fn main() {
    let value = json!({
        "name": "Alice",
        "age": 30,
        "active": true
    });
    let _ = value;
}
