// Pass: Basic derive Query usage
use mik_sdk_macros::Query;

#[derive(Query)]
pub struct ListQuery {
    #[field(default = 1)]
    pub page: u32,
    #[field(default = 20)]
    pub limit: u32,
}

fn main() {}
