use mik_sdk_macros::fetch;

// Error: fetch! requires URL
fn main() {
    let _req = fetch!(GET);
}
