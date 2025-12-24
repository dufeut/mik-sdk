use mik_sdk_macros::routes;

fn home() -> String { String::new() }

// Error: Invalid HTTP method
routes! {
    INVALID "/users" => home,
}

fn main() {}
