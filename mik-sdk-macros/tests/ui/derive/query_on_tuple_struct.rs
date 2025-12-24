use mik_sdk_macros::Query;

// Error: Query derive only supports structs with named fields
#[derive(Query)]
struct QueryTuple(String);

fn main() {}
