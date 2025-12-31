use mik_sdk::fetch;
fn main() {
    let request = {
        ::mik_sdk::http_client::ClientRequest::new(
            ::mik_sdk::http_client::Method::Get,
            &"https://api.example.com/users",
        )
    };
    let _ = request;
}
