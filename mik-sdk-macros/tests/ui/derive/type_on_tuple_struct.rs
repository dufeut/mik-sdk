use mik_sdk_macros::Type;

// Error: Type derive only supports structs with named fields
#[derive(Type)]
struct MyTuple(String, i32);

fn main() {}
