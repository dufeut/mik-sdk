use mik_sdk_macros::ok;

// Error: ok! macro requires a body
fn main() {
    let _resp = ok!();
}
