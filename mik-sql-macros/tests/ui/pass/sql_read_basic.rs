// Pass: Basic sql_read! usage
use mik_sql_macros::sql_read;

fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id, name, email],
    });
}
