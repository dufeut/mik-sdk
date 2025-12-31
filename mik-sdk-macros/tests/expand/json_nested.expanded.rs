use mik_sdk::json;
fn main() {
    let value = json::obj()
        .set(
            "user",
            json::obj()
                .set("name", json::str("Bob"))
                .set("address", json::obj().set("city", json::str("NYC"))),
        )
        .set(
            "tags",
            json::arr().push(json::str("a")).push(json::str("b")).push(json::str("c")),
        );
    let _ = value;
}
