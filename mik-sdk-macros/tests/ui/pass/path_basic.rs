// Pass: Basic derive Path usage
use mik_sdk_macros::Path;

#[derive(Path)]
pub struct UserPath {
    pub id: String,
}

fn main() {}
