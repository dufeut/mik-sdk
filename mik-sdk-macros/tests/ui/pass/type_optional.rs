// Pass: derive Type with optional fields
use mik_sdk_macros::Type;

#[derive(Type)]
pub struct Profile {
    pub name: String,
    pub bio: Option<String>,
    pub age: Option<i32>,
}

fn main() {}
