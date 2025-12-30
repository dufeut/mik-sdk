// Pass: Basic sql_create! usage
use mik_sql_macros::sql_create;

fn main() {
    let name = "Alice";
    let email = "alice@example.com";
    let (_sql, _params) = sql_create!(users {
        name: str(name),
        email: str(email),
    });
}
