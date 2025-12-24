use mik_sdk_macros::Type;

// Error: Type derive only supports structs, not enums
#[derive(Type)]
enum MyEnum {
    A,
    B,
}

fn main() {}
