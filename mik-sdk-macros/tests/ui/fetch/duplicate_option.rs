use mik_sdk_macros::fetch;

// Error: Duplicate headers option
fn main() {
    let _req = fetch!(GET "https://example.com",
        headers: { "A": "1" },
        headers: { "B": "2" }
    );
}
