// Pass: Basic sql_delete! usage
use mik_sql_macros::sql_delete;

fn main() {
    let user_id = 1;
    let (_sql, _params) = sql_delete!(users {
        filter: { id: int(user_id) },
    });
}
