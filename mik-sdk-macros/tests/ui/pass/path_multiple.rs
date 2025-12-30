// Pass: derive Path with multiple path parameters
use mik_sdk_macros::Path;

#[derive(Path)]
pub struct OrgUserPath {
    pub org_id: String,
    pub user_id: String,
}

fn main() {}
