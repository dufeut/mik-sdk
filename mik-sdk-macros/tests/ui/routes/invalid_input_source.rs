use mik_sdk_macros::routes;

fn handler() -> String { String::new() }

// Error: Invalid input source (should be path, body, or query)
routes! {
    GET "/users" => handler(data: UserType),
}

fn main() {}
