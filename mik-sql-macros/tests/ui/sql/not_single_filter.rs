use mik_sql_macros::sql_read;

// Error: $not operator requires exactly 1 filter
fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id],
        filter: { $not: { a: 1, b: 2 } },
    });
}
