use mik_sql_macros::sql_read;

// Error: Unknown option 'invalid_option'
fn main() {
    let (_sql, _params) = sql_read!(users {
        select: [id, name],
        invalid_option: true,
    });
}
