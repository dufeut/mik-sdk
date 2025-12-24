use mik_sdk_macros::routes;

fn home() -> String { String::new() }

// Error: Expected '=>' after route pattern
routes! {
    "/users" home,
}

fn main() {}
