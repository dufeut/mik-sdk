use mik_sdk_macros::ensure;

fn find_user() -> Option<String> { None }

// Error: Missing status code
fn main() {
    let _user = ensure!(find_user(), "User not found");
}
