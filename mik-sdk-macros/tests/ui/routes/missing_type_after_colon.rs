use mik_sdk_macros::routes;

fn handler() -> String { String::new() }

// Error: Missing type name after input source
routes! {
    GET "/users/{id}" => handler(path:),
}

fn main() {}
