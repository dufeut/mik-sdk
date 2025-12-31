use mik_sdk::json;
fn main() {
    let value = json::obj()
        .set("name", json::str("Alice"))
        .set("age", json::int(30 as i64))
        .set("active", json::bool(true));
    let _ = value;
}
