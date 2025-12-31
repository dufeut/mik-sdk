// Test fetch! macro expansion for GET request
use mik_sdk::fetch;

fn main() {
    let request = fetch!(GET "https://api.example.com/users");
    let _ = request;
}
