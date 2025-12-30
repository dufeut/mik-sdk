// Pass: sql_read! with pagination
use mik_sql_macros::sql_read;

fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id, name],
        order: name,
        limit: 10,
        offset: 0,
    });
}
