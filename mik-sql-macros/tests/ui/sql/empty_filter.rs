use mik_sql_macros::sql_read;

// Error: Empty filter block
fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id],
        filter: {},
    });
}
