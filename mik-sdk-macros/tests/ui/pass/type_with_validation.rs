// Pass: derive Type with validation attributes
use mik_sdk_macros::Type;

#[derive(Type)]
pub struct Constrained {
    #[field(min = 1, max = 100)]
    pub value: i32,
    #[field(min = 3, max = 50)]
    pub name: String,
}

fn main() {}
