// Pass: derive Query with optional fields
use mik_sdk_macros::Query;

#[derive(Query)]
pub struct SearchQuery {
    pub q: Option<String>,
    #[field(default = 1)]
    pub page: u32,
    #[field(default = 10)]
    pub limit: u32,
}

fn main() {}
