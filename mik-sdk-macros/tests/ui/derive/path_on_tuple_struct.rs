use mik_sdk_macros::Path;

// Error: Path derive only supports structs with named fields
#[derive(Path)]
struct PathTuple(String);

fn main() {}
