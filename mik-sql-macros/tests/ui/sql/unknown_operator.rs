use mik_sql_macros::sql_read;

// Error: Unknown operator '$unknown'
fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id],
        filter: { name: { $unknown: "test" } },
    });
}
