use mik_sql_macros::sql_read;

// Error: Unknown aggregate function 'invalid_func'
fn main() {
    let (_sql, _params) = sql_read!(users {
        aggregate: { total: invalid_func(id) },
    });
}
