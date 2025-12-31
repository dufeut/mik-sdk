use mik_sdk::fetch;
fn main() {
    let request = {
        ::mik_sdk::http_client::ClientRequest::new(
                ::mik_sdk::http_client::Method::Post,
                &"https://api.example.com/users",
            )
            .json(
                &json::obj()
                    .set("name", json::str("Alice"))
                    .set("email", json::str("alice@example.com"))
                    .to_bytes(),
            )
    };
    let _ = request;
}
