use mik_sdk_macros::routes;

fn home() -> String { String::new() }

// Error: Handler name should not be quoted
routes! {
    "/users" => "home",
}

fn main() {}
