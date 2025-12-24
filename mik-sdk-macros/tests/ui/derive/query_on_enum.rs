use mik_sdk_macros::Query;

// Error: Query derive only supports structs
#[derive(Query)]
enum SearchParams {
    Simple,
    Complex,
}

fn main() {}
