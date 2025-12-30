// Pass: Basic sql_update! usage
use mik_sql_macros::sql_update;

fn main() {
    let user_id = 1;
    let new_name = "Updated Name";
    let (_sql, _params) = sql_update!(users {
        set: { name: str(new_name) },
        filter: { id: int(user_id) },
    });
}
