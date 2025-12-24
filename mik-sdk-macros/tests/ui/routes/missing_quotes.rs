use mik_sdk_macros::routes;

fn home() -> String { String::new() }

// Error: Route pattern must be a string literal
routes! {
    /users => home,
}

fn main() {}
