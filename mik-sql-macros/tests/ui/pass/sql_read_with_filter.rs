// Pass: sql_read! with filter
use mik_sql_macros::sql_read;

fn main() {
    let user_id = 42;
    let (_sql, _params) = sql_read!(users {
        select: [id, name, email],
        filter: { id: int(user_id) },
    });
}
