use mik_sdk_macros::error;

// Error: error! macro requires 'status' field
fn main() {
    let _resp = error! {
        title: "Not Found",
    };
}
