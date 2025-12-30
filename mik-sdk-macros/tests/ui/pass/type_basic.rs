// Pass: Basic derive Type usage on a struct with named fields
use mik_sdk_macros::Type;

#[derive(Type)]
pub struct User {
    pub name: String,
    pub age: i32,
}

fn main() {}
