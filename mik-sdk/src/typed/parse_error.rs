//! ParseError enum and implementations.

use super::ValidationError;

/// Error returned when parsing input fails.
///
/// This covers structural errors like missing required fields or type mismatches.
/// Errors are represented as distinct enum variants for pattern matching.
///
/// # Example
///
/// ```ignore
/// match error {
///     ParseError::MissingField { field } => println!("Missing: {}", field),
///     ParseError::TypeMismatch { field, expected } => {
///         println!("Expected {} for {}", expected, field)
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
pub enum ParseError {
    /// A required field is missing from the input.
    MissingField {
        /// The name of the missing field
        field: String,
    },

    /// The field value has an invalid format (e.g., "abc" for an integer).
    InvalidFormat {
        /// The name of the field
        field: String,
        /// The invalid value that was provided
        value: String,
    },

    /// The field value has the wrong type (e.g., string instead of number).
    TypeMismatch {
        /// The name of the field
        field: String,
        /// The expected type (e.g., "integer", "string", "boolean")
        expected: String,
    },

    /// A custom parse error with a user-defined message.
    Custom {
        /// The name of the field (or empty for general errors)
        field: String,
        /// Custom error message
        message: String,
    },
}

impl ParseError {
    /// Create an error for a missing required field.
    #[inline]
    #[must_use]
    pub fn missing(field: &str) -> Self {
        Self::MissingField {
            field: field.to_string(),
        }
    }

    /// Create an error for an invalid format.
    #[inline]
    #[must_use]
    pub fn invalid_format(field: &str, value: &str) -> Self {
        Self::InvalidFormat {
            field: field.to_string(),
            value: value.to_string(),
        }
    }

    /// Create an error for a type mismatch.
    #[inline]
    #[must_use]
    pub fn type_mismatch(field: &str, expected: &str) -> Self {
        Self::TypeMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
        }
    }

    /// Create a custom parse error.
    #[inline]
    #[must_use]
    pub fn custom(field: &str, message: impl Into<String>) -> Self {
        Self::Custom {
            field: field.to_string(),
            message: message.into(),
        }
    }

    /// Get the field name associated with this error.
    #[inline]
    #[must_use]
    pub fn field(&self) -> &str {
        match self {
            Self::MissingField { field }
            | Self::InvalidFormat { field, .. }
            | Self::TypeMismatch { field, .. }
            | Self::Custom { field, .. } => field,
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
    /// let err = ParseError::missing("city").with_path("address");
    /// assert_eq!(err.field(), "address.city");
    /// ```
    #[must_use]
    pub fn with_path(self, parent: &str) -> Self {
        match self {
            Self::MissingField { field } => Self::MissingField {
                field: format!("{parent}.{field}"),
            },
            Self::InvalidFormat { field, value } => Self::InvalidFormat {
                field: format!("{parent}.{field}"),
                value,
            },
            Self::TypeMismatch { field, expected } => Self::TypeMismatch {
                field: format!("{parent}.{field}"),
                expected,
            },
            Self::Custom { field, message } => Self::Custom {
                field: format!("{parent}.{field}"),
                message,
            },
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => {
                write!(f, "Missing required field: {field}")
            },
            Self::InvalidFormat { field, value } => {
                write!(f, "Invalid format for '{field}': {value}")
            },
            Self::TypeMismatch { field, expected } => {
                write!(f, "Expected {expected} for field '{field}'")
            },
            Self::Custom { message, .. } => {
                write!(f, "{message}")
            },
        }
    }
}

impl std::error::Error for ParseError {}

/// Convert ValidationError to ParseError.
///
/// This allows using `?` to propagate validation errors in contexts
/// that expect parse errors.
impl From<ValidationError> for ParseError {
    fn from(err: ValidationError) -> Self {
        Self::Custom {
            field: err.field().to_string(),
            message: err.to_string(),
        }
    }
}
