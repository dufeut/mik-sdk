// Test fetch! macro expansion for POST with JSON body
use mik_sdk::fetch;

fn main() {
    let request = fetch!(POST "https://api.example.com/users", json: {
        "name": "Alice",
        "email": "alice@example.com"
    });
    let _ = request;
}
