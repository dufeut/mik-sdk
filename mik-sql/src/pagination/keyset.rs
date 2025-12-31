//! Keyset pagination condition generation.

use crate::builder::{
    CompoundFilter, Filter, FilterExpr, LogicalOp, Operator, SortDir, SortField, Value,
};

use super::cursor::Cursor;

/// Keyset pagination condition.
///
/// Generates efficient `(col1, col2) > ($1, $2)` style WHERE clauses
/// for keyset/seek pagination.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct KeysetCondition {
    /// The sort fields and their directions.
    pub sort_fields: Vec<SortField>,
    /// The cursor values for each field.
    pub cursor_values: Vec<Value>,
    /// Direction: true for "after", false for "before".
    pub forward: bool,
}

impl KeysetCondition {
    /// Create a new keyset condition for paginating after a cursor.
    #[must_use]
    pub fn after(sorts: &[SortField], cursor: &Cursor) -> Option<Self> {
        Self::new(sorts, cursor, true)
    }

    /// Create a new keyset condition for paginating before a cursor.
    #[must_use]
    pub fn before(sorts: &[SortField], cursor: &Cursor) -> Option<Self> {
        Self::new(sorts, cursor, false)
    }

    fn new(sorts: &[SortField], cursor: &Cursor, forward: bool) -> Option<Self> {
        if sorts.is_empty() {
            return None;
        }

        // Match cursor fields to sort fields
        let mut cursor_values = Vec::new();
        for sort in sorts {
            let value = cursor
                .fields
                .iter()
                .find(|(name, _)| name == &sort.field)
                .map(|(_, v)| v.clone())?;
            cursor_values.push(value);
        }

        Some(Self {
            sort_fields: sorts.to_vec(),
            cursor_values,
            forward,
        })
    }

    /// Convert to a filter expression for the query builder.
    ///
    /// For a single field, generates: `field > $1` (or `<` for DESC)
    ///
    /// For multiple fields, generates proper compound OR conditions:
    /// `(a, b) > (1, 2)` becomes: `(a > 1) OR (a = 1 AND b > 2)`
    ///
    /// For 3+ fields: `(a > 1) OR (a = 1 AND b > 2) OR (a = 1 AND b = 2 AND c > 3)`
    ///
    /// This follows the keyset pagination standard used by PostgreSQL, GraphQL Relay,
    /// and major ORMs. See: <https://use-the-index-luke.com/no-offset>
    #[must_use]
    pub fn to_filter_expr(&self) -> FilterExpr {
        if self.sort_fields.is_empty() || self.cursor_values.is_empty() {
            // Return a tautology (always true) - will be optimized away
            return FilterExpr::Simple(Filter {
                field: "1".to_string(),
                op: Operator::Eq,
                value: Value::Int(1),
            });
        }

        if self.sort_fields.len() == 1 {
            // Simple case: single field comparison
            let sort = &self.sort_fields[0];
            let value = &self.cursor_values[0];
            let op = self.get_operator(sort.dir);

            return FilterExpr::Simple(Filter {
                field: sort.field.clone(),
                op,
                value: value.clone(),
            });
        }

        // Multi-field keyset: generate OR conditions
        // (a, b, c) > (1, 2, 3) expands to:
        //   (a > 1)
        //   OR (a = 1 AND b > 2)
        //   OR (a = 1 AND b = 2 AND c > 3)
        let mut or_conditions: Vec<FilterExpr> = Vec::new();

        for i in 0..self.sort_fields.len() {
            // Build: equality on fields 0..i, then comparison on field i
            let mut and_conditions: Vec<FilterExpr> = Vec::new();

            // Add equality conditions for all preceding fields
            for j in 0..i {
                and_conditions.push(FilterExpr::Simple(Filter {
                    field: self.sort_fields[j].field.clone(),
                    op: Operator::Eq,
                    value: self.cursor_values[j].clone(),
                }));
            }

            // Add comparison condition for current field
            let sort = &self.sort_fields[i];
            let value = &self.cursor_values[i];
            let op = self.get_operator(sort.dir);
            and_conditions.push(FilterExpr::Simple(Filter {
                field: sort.field.clone(),
                op,
                value: value.clone(),
            }));

            // Combine with AND
            let condition = if and_conditions.len() == 1 {
                and_conditions.into_iter().next().unwrap()
            } else {
                FilterExpr::Compound(CompoundFilter {
                    op: LogicalOp::And,
                    filters: and_conditions,
                })
            };

            or_conditions.push(condition);
        }

        // Combine all with OR
        if or_conditions.len() == 1 {
            or_conditions.into_iter().next().unwrap()
        } else {
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::Or,
                filters: or_conditions,
            })
        }
    }

    const fn get_operator(&self, dir: SortDir) -> Operator {
        match (self.forward, dir) {
            (true, SortDir::Asc) => Operator::Gt,
            (true, SortDir::Desc) => Operator::Lt,
            (false, SortDir::Asc) => Operator::Lt,
            (false, SortDir::Desc) => Operator::Gt,
        }
    }
}

#[cfg(test)]
#[allow(clippy::match_wildcard_for_single_variants)]
mod tests {
    use super::*;

    #[test]
    fn test_keyset_condition_asc() {
        let sorts = vec![SortField::new("id", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.field, "id");
                assert_eq!(f.op, Operator::Gt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_desc() {
        let sorts = vec![SortField::new("created_at", SortDir::Desc)];
        let cursor = Cursor::new().string("created_at", "2024-01-01");

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.op, Operator::Lt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_before() {
        let sorts = vec![SortField::new("id", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::before(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.op, Operator::Lt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_multi_field_asc_asc() {
        // Test: (created_at, id) > ('2024-01-01', 100)
        // Should generate: (created_at > '2024-01-01') OR (created_at = '2024-01-01' AND id > 100)
        let sorts = vec![
            SortField::new("created_at", SortDir::Asc),
            SortField::new("id", SortDir::Asc),
        ];
        let cursor = Cursor::new()
            .string("created_at", "2024-01-01")
            .int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        // Should be OR compound
        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);
                assert_eq!(compound.filters.len(), 2);

                // First: created_at > '2024-01-01'
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "created_at");
                        assert_eq!(f.op, Operator::Gt);
                    },
                    _ => panic!("Expected simple filter for first condition"),
                }

                // Second: (created_at = '2024-01-01' AND id > 100)
                match &compound.filters[1] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.op, LogicalOp::And);
                        assert_eq!(and_compound.filters.len(), 2);
                    },
                    _ => panic!("Expected compound AND filter for second condition"),
                }
            },
            _ => panic!("Expected compound OR filter for multi-field keyset"),
        }
    }

    #[test]
    fn test_keyset_condition_multi_field_desc_asc() {
        // Test: ORDER BY created_at DESC, id ASC with cursor after
        let sorts = vec![
            SortField::new("created_at", SortDir::Desc),
            SortField::new("id", SortDir::Asc),
        ];
        let cursor = Cursor::new()
            .string("created_at", "2024-01-01")
            .int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);

                // First condition: created_at < '2024-01-01' (DESC means <)
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "created_at");
                        assert_eq!(f.op, Operator::Lt); // DESC + After = Lt
                    },
                    _ => panic!("Expected simple filter"),
                }
            },
            _ => panic!("Expected compound filter"),
        }
    }

    #[test]
    fn test_keyset_condition_three_fields() {
        // Test: (a, b, c) > (1, 2, 3) expands to:
        //   (a > 1)
        //   OR (a = 1 AND b > 2)
        //   OR (a = 1 AND b = 2 AND c > 3)
        let sorts = vec![
            SortField::new("a", SortDir::Asc),
            SortField::new("b", SortDir::Asc),
            SortField::new("c", SortDir::Asc),
        ];
        let cursor = Cursor::new().int("a", 1).int("b", 2).int("c", 3);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);
                assert_eq!(compound.filters.len(), 3);

                // First: a > 1 (simple)
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "a");
                        assert_eq!(f.op, Operator::Gt);
                    },
                    _ => panic!("Expected simple filter"),
                }

                // Second: a = 1 AND b > 2
                match &compound.filters[1] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.filters.len(), 2);
                    },
                    _ => panic!("Expected compound filter"),
                }

                // Third: a = 1 AND b = 2 AND c > 3
                match &compound.filters[2] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.filters.len(), 3);
                    },
                    _ => panic!("Expected compound filter"),
                }
            },
            _ => panic!("Expected compound filter"),
        }
    }

    #[test]
    fn test_keyset_with_missing_cursor_field() {
        // Sort by field not in cursor should return None
        let sorts = vec![SortField::new("missing_field", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor);
        assert!(
            condition.is_none(),
            "Should return None when cursor missing required field"
        );
    }

    #[test]
    fn test_keyset_with_empty_sorts() {
        let cursor = Cursor::new().int("id", 100);
        let condition = KeysetCondition::after(&[], &cursor);
        assert!(
            condition.is_none(),
            "Should return None for empty sort list"
        );
    }
}
