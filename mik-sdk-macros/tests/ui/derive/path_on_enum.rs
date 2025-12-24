use mik_sdk_macros::Path;

// Error: Path derive only supports structs
#[derive(Path)]
enum PathParams {
    User,
    Post,
}

fn main() {}
