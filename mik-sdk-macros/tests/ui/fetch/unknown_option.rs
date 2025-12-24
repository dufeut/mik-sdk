use mik_sdk_macros::fetch;

// Error: Unknown option 'invalid_opt'
fn main() {
    let _req = fetch!(GET "https://example.com", invalid_opt: true);
}
