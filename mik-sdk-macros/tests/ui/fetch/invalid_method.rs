use mik_sdk_macros::fetch;

// Error: Invalid HTTP method
fn main() {
    let _req = fetch!(INVALID "https://example.com");
}
