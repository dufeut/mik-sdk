// Pass: sql_update! with returning clause
use mik_sql_macros::sql_update;

fn main() {
    let user_id = 1;
    let new_email = "new@example.com";
    let (_sql, _params) = sql_update!(users {
        set: { email: str(new_email) },
        filter: { id: int(user_id) },
        returning: [id, email, updated_at],
    });
}
