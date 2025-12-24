use mik_sdk_macros::fetch;

// Error: Cannot specify both json and body
fn main() {
    let data = b"raw data";
    let _req = fetch!(POST "https://example.com",
        json: { "key": "value" },
        body: data
    );
}
