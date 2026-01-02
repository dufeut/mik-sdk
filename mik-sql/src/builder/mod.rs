//! Query builder for SQL generation with parameterization.

mod delete;
mod filter;
mod insert;
mod parse;
mod select;
mod types;
mod update;

// Re-export all public items
pub use delete::{DeleteBuilder, delete, delete_sqlite};
pub use insert::{InsertBuilder, insert, insert_sqlite};
pub use parse::{ParseError, parse_filter};
pub use select::QueryBuilder;
pub use types::{
    Aggregate, AggregateFunc, CompoundFilter, ComputedField, CursorDirection, Filter, FilterExpr,
    LogicalOp, Operator, QueryResult, SortDir, SortField, Value, and, not, or, simple,
};
pub use update::{UpdateBuilder, update, update_sqlite};
