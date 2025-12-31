//! PageInfo struct and methods for pagination responses.

use super::cursor::Cursor;

/// Page information for paginated responses.
///
/// # Example
///
/// ```
/// # use mik_sql::PageInfo;
/// let page_info = PageInfo::new(20, 20)
///     .with_next_cursor(Some("abc123".to_string()))
///     .with_prev_cursor(Some("xyz789".to_string()))
///     .with_total(100);
///
/// assert!(page_info.has_next);
/// assert!(page_info.has_prev);
/// assert_eq!(page_info.total, Some(100));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PageInfo {
    /// Whether there are more items after this page.
    pub has_next: bool,
    /// Whether there are items before this page.
    pub has_prev: bool,
    /// Cursor to fetch the next page.
    pub next_cursor: Option<String>,
    /// Cursor to fetch the previous page.
    pub prev_cursor: Option<String>,
    /// Total count (if available).
    pub total: Option<u64>,
}

impl PageInfo {
    /// Create page info based on returned count vs requested limit.
    ///
    /// If `count >= limit`, assumes there are more items.
    #[must_use]
    pub const fn new(count: usize, limit: usize) -> Self {
        Self {
            has_next: count >= limit,
            has_prev: false,
            next_cursor: None,
            prev_cursor: None,
            total: None,
        }
    }

    /// Set whether there are previous items.
    #[must_use]
    pub const fn with_has_prev(mut self, has_prev: bool) -> Self {
        self.has_prev = has_prev;
        self
    }

    /// Set the next cursor.
    #[must_use]
    pub fn with_next_cursor(mut self, cursor: Option<String>) -> Self {
        self.next_cursor = cursor;
        if self.next_cursor.is_some() {
            self.has_next = true;
        }
        self
    }

    /// Set the previous cursor.
    #[must_use]
    pub fn with_prev_cursor(mut self, cursor: Option<String>) -> Self {
        self.prev_cursor = cursor;
        if self.prev_cursor.is_some() {
            self.has_prev = true;
        }
        self
    }

    /// Set the total count.
    #[must_use]
    pub const fn with_total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }

    /// Create cursor from the last item using a builder function.
    #[allow(clippy::single_option_map)] // Intentional API design
    pub fn cursor_from<T, F>(item: Option<&T>, builder: F) -> Option<String>
    where
        F: FnOnce(&T) -> Cursor,
    {
        item.map(|item| builder(item).encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_info_basic() {
        let info = PageInfo::new(20, 20);
        assert!(info.has_next);
        assert!(!info.has_prev);

        let info = PageInfo::new(15, 20);
        assert!(!info.has_next);
    }

    #[test]
    fn test_page_info_with_cursors() {
        let info = PageInfo::new(20, 20)
            .with_next_cursor(Some("abc".to_string()))
            .with_prev_cursor(Some("xyz".to_string()))
            .with_total(100);

        assert!(info.has_next);
        assert!(info.has_prev);
        assert_eq!(info.next_cursor, Some("abc".to_string()));
        assert_eq!(info.prev_cursor, Some("xyz".to_string()));
        assert_eq!(info.total, Some(100));
    }
}
