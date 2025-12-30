// Pass: sql_create! with returning clause
use mik_sql_macros::sql_create;

fn main() {
    let name = "Bob";
    let (_sql, _params) = sql_create!(users {
        name: str(name),
        returning: [id, created_at],
    });
}
