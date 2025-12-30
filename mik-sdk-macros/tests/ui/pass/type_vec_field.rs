// Pass: derive Type with Vec field
use mik_sdk_macros::Type;

#[derive(Type)]
pub struct Tags {
    pub items: Vec<String>,
    pub counts: Vec<i32>,
}

fn main() {}
