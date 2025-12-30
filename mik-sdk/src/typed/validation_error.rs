//! ValidationError enum and implementations.

/// Error returned when constraint validation fails.
///
/// This covers constraint violations like min/max length, format, pattern.
/// Errors are represented as distinct enum variants for pattern matching.
///
/// # Example
///
/// ```ignore
/// match error {
///     ValidationError::Min { field, min } => {
///         println!("{} must be at least {}", field, min)
///     }
///     ValidationError::Max { field, max } => {
///         println!("{} must be at most {}", field, max)
///     }
///     _ => println!("{}", error),
/// }
/// ```
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new variants
/// in future versions without breaking existing match expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationError {
    /// Value is below the minimum constraint.
    Min {
        /// The name of the field
        field: String,
        /// The minimum allowed value
        min: i64,
    },

    /// Value exceeds the maximum constraint.
    Max {
        /// The name of the field
        field: String,
        /// The maximum allowed value
        max: i64,
    },

    /// Value doesn't match the required pattern.
    Pattern {
        /// The name of the field
        field: String,
        /// The pattern that should be matched
        pattern: String,
    },

    /// Value doesn't match the required format (e.g., email, uuid).
    Format {
        /// The name of the field
        field: String,
        /// The expected format (e.g., "email", "uuid", "date-time")
        expected: String,
    },

    /// A custom validation error with a user-defined message.
    Custom {
        /// The name of the field
        field: String,
        /// The constraint that was violated
        constraint: String,
        /// Custom error message
        message: String,
    },
}

impl ValidationError {
    /// Create an error for a minimum constraint violation.
    #[inline]
    #[must_use]
    pub fn min(field: &str, min: i64) -> Self {
        Self::Min {
            field: field.to_string(),
            min,
        }
    }

    /// Create an error for a maximum constraint violation.
    #[inline]
    #[must_use]
    pub fn max(field: &str, max: i64) -> Self {
        Self::Max {
            field: field.to_string(),
            max,
        }
    }

    /// Create an error for a pattern constraint violation.
    #[inline]
    #[must_use]
    pub fn pattern(field: &str, pattern: &str) -> Self {
        Self::Pattern {
            field: field.to_string(),
            pattern: pattern.to_string(),
        }
    }

    /// Create an error for a format constraint violation.
    #[inline]
    #[must_use]
    pub fn format(field: &str, expected_format: &str) -> Self {
        Self::Format {
            field: field.to_string(),
            expected: expected_format.to_string(),
        }
    }

    /// Create a custom validation error.
    #[inline]
    #[must_use]
    pub fn custom(field: &str, constraint: &str, message: impl Into<String>) -> Self {
        Self::Custom {
            field: field.to_string(),
            constraint: constraint.to_string(),
            message: message.into(),
        }
    }

    /// Get the field name associated with this error.
    #[inline]
    #[must_use]
    pub fn field(&self) -> &str {
        match self {
            Self::Min { field, .. }
            | Self::Max { field, .. }
            | Self::Pattern { field, .. }
            | Self::Format { field, .. }
            | Self::Custom { field, .. } => field,
        }
    }

    /// Get the constraint name for this error.
    #[inline]
    #[must_use]
    pub fn constraint(&self) -> &str {
        match self {
            Self::Min { .. } => "min",
            Self::Max { .. } => "max",
            Self::Pattern { .. } => "pattern",
            Self::Format { .. } => "format",
            Self::Custom { constraint, .. } => constraint,
        }
    }

    /// Get the error message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Add parent field context to the error.
    ///
    /// Useful for nested types where you want to show the full path
    /// to the field that failed (e.g., "user.address.city").
    ///
    /// # Example
    ///
    /// ```ignore
    /// let err = ValidationError::min("count", 1).with_path("items");
    /// assert_eq!(err.field(), "items.count");
    /// ```
    #[must_use]
    pub fn with_path(self, parent: &str) -> Self {
        match self {
            Self::Min { field, min } => Self::Min {
                field: format!("{parent}.{field}"),
                min,
            },
            Self::Max { field, max } => Self::Max {
                field: format!("{parent}.{field}"),
                max,
            },
            Self::Pattern { field, pattern } => Self::Pattern {
                field: format!("{parent}.{field}"),
                pattern,
            },
            Self::Format { field, expected } => Self::Format {
                field: format!("{parent}.{field}"),
                expected,
            },
            Self::Custom {
                field,
                constraint,
                message,
            } => Self::Custom {
                field: format!("{parent}.{field}"),
                constraint,
                message,
            },
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Min { field, min } => {
                write!(f, "'{field}' must be at least {min}")
            },
            Self::Max { field, max } => {
                write!(f, "'{field}' must be at most {max}")
            },
            Self::Pattern { field, pattern } => {
                write!(f, "'{field}' must match pattern: {pattern}")
            },
            Self::Format { field, expected } => {
                write!(f, "'{field}' must be a valid {expected}")
            },
            Self::Custom { message, .. } => {
                write!(f, "{message}")
            },
        }
    }
}

impl std::error::Error for ValidationError {}
