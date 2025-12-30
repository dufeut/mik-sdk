//! Environment variable access for WASI HTTP handlers.
//!
//! This module provides ergonomic wrappers around `wasi:cli/environment` for
//! accessing environment variables in handler code.
//!
//! # Usage
//!
//! ```
//! # use mik_sdk::env;
//! // Mock environment for demonstration
//! let env_vars = vec![
//!     ("PORT".to_string(), "3000".to_string()),
//!     ("DEBUG".to_string(), "true".to_string()),
//! ];
//!
//! let port = env::get_or(&env_vars, "PORT", "8080");
//! assert_eq!(port, "3000");
//!
//! let debug = env::bool(&env_vars, "DEBUG", false);
//! assert!(debug);
//! ```
//!
//! # Caching
//!
//! For better performance, cache the environment on first access:
//!
//! ```
//! # use mik_sdk::env::EnvCache;
//! let env_vars = vec![
//!     ("PORT".to_string(), "3000".to_string()),
//! ];
//! let cache = EnvCache::new(env_vars);
//!
//! let port = cache.get_or("PORT", "8080");
//! assert_eq!(port, "3000");
//! ```

use std::collections::HashMap;

/// Get an environment variable by name.
///
/// Returns `None` if the variable is not set.
///
/// # Examples
///
/// ```
/// # use mik_sdk::env;
/// let env_vars = vec![
///     ("DATABASE_URL".to_string(), "postgres://localhost/db".to_string()),
/// ];
///
/// let db_url = env::get(&env_vars, "DATABASE_URL");
/// assert_eq!(db_url, Some("postgres://localhost/db".to_string()));
///
/// let missing = env::get(&env_vars, "NONEXISTENT");
/// assert_eq!(missing, None);
/// ```
#[must_use]
pub fn get(env: &[(String, String)], name: &str) -> Option<String> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// Get an environment variable or return a default value.
///
/// # Examples
///
/// ```
/// # use mik_sdk::env;
/// let env_vars = vec![
///     ("PORT".to_string(), "3000".to_string()),
/// ];
///
/// let port = env::get_or(&env_vars, "PORT", "8080");
/// assert_eq!(port, "3000");
///
/// let host = env::get_or(&env_vars, "HOST", "0.0.0.0");
/// assert_eq!(host, "0.0.0.0"); // Uses default
/// ```
#[must_use]
pub fn get_or(env: &[(String, String)], name: &str, default: &str) -> String {
    get(env, name).unwrap_or_else(|| default.to_string())
}

/// Get an environment variable as a boolean.
///
/// Returns `true` if the value is "true", "1", or "yes" (case-insensitive).
/// Returns the default if the variable is not set.
///
/// # Examples
///
/// ```
/// # use mik_sdk::env;
/// let env_vars = vec![
///     ("DEBUG".to_string(), "true".to_string()),
///     ("VERBOSE".to_string(), "1".to_string()),
///     ("ENABLED".to_string(), "false".to_string()),
/// ];
///
/// assert!(env::bool(&env_vars, "DEBUG", false));
/// assert!(env::bool(&env_vars, "VERBOSE", false));
/// assert!(!env::bool(&env_vars, "ENABLED", true)); // "false" is not truthy
/// assert!(!env::bool(&env_vars, "MISSING", false)); // Uses default
/// ```
#[must_use]
pub fn bool(env: &[(String, String)], name: &str, default: bool) -> bool {
    get(env, name).map_or(default, |v| {
        let v_lower = v.to_lowercase();
        v_lower == "true" || v_lower == "1" || v_lower == "yes"
    })
}

/// Get all environment variables as a cloned vector.
///
/// # Examples
///
/// ```
/// # use mik_sdk::env;
/// let env_vars = vec![
///     ("PORT".to_string(), "3000".to_string()),
///     ("HOST".to_string(), "localhost".to_string()),
/// ];
///
/// let all = env::all(&env_vars);
/// assert_eq!(all.len(), 2);
/// ```
#[must_use]
pub fn all(env: &[(String, String)]) -> Vec<(String, String)> {
    env.to_vec()
}

/// Environment variable cache for efficient repeated access.
///
/// Uses a `HashMap` for O(1) lookups instead of linear search.
///
/// # Examples
///
/// ```
/// # use mik_sdk::env::EnvCache;
/// let env_vars = vec![
///     ("PORT".to_string(), "3000".to_string()),
///     ("DEBUG".to_string(), "true".to_string()),
/// ];
/// let cache = EnvCache::new(env_vars);
///
/// assert_eq!(cache.get("PORT"), Some("3000".to_string()));
/// assert_eq!(cache.get_or("PORT", "8080"), "3000");
/// assert_eq!(cache.get_or("MISSING", "default"), "default");
/// assert!(cache.bool("DEBUG", false));
/// assert_eq!(cache.all().len(), 2);
/// ```
#[derive(Debug)]
pub struct EnvCache {
    map: HashMap<String, String>,
    // Keep original vec for all() method
    vec: Vec<(String, String)>,
}

impl EnvCache {
    /// Create a new environment cache from the current environment.
    #[must_use]
    pub fn new(env: Vec<(String, String)>) -> Self {
        let map = env.iter().cloned().collect();
        Self { map, vec: env }
    }

    /// Get an environment variable by name. O(1) lookup.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        self.map.get(name).cloned()
    }

    /// Get an environment variable or return a default value.
    #[must_use]
    pub fn get_or(&self, name: &str, default: &str) -> String {
        self.map
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    /// Get an environment variable as a boolean.
    #[must_use]
    pub fn bool(&self, name: &str, default: bool) -> bool {
        self.map.get(name).map_or(default, |v| {
            let v_lower = v.to_lowercase();
            v_lower == "true" || v_lower == "1" || v_lower == "yes"
        })
    }

    /// Get all environment variables.
    #[must_use]
    pub fn all(&self) -> &[(String, String)] {
        &self.vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_env() -> Vec<(String, String)> {
        vec![
            ("PORT".to_string(), "3000".to_string()),
            ("HOST".to_string(), "localhost".to_string()),
            ("DEBUG".to_string(), "true".to_string()),
            ("VERBOSE".to_string(), "1".to_string()),
            ("QUIET".to_string(), "yes".to_string()),
            ("ENABLED".to_string(), "false".to_string()),
        ]
    }

    #[test]
    fn test_get() {
        let env = mock_env();
        assert_eq!(get(&env, "PORT"), Some("3000".to_string()));
        assert_eq!(get(&env, "HOST"), Some("localhost".to_string()));
        assert_eq!(get(&env, "NONEXISTENT"), None);
    }

    #[test]
    fn test_get_or() {
        let env = mock_env();
        assert_eq!(get_or(&env, "PORT", "8080"), "3000");
        assert_eq!(get_or(&env, "NONEXISTENT", "default"), "default");
    }

    #[test]
    fn test_bool() {
        let env = mock_env();
        assert!(bool(&env, "DEBUG", false));
        assert!(bool(&env, "VERBOSE", false));
        assert!(bool(&env, "QUIET", false));
        assert!(!bool(&env, "ENABLED", true));
        assert!(!bool(&env, "NONEXISTENT", false));
        assert!(bool(&env, "NONEXISTENT", true));
    }

    #[test]
    fn test_bool_case_insensitive() {
        let env = vec![
            ("TRUE_UPPER".to_string(), "TRUE".to_string()),
            ("TRUE_LOWER".to_string(), "true".to_string()),
            ("TRUE_MIXED".to_string(), "TrUe".to_string()),
            ("YES_UPPER".to_string(), "YES".to_string()),
            ("ONE".to_string(), "1".to_string()),
        ];

        assert!(bool(&env, "TRUE_UPPER", false));
        assert!(bool(&env, "TRUE_LOWER", false));
        assert!(bool(&env, "TRUE_MIXED", false));
        assert!(bool(&env, "YES_UPPER", false));
        assert!(bool(&env, "ONE", false));
    }

    #[test]
    fn test_all() {
        let env = mock_env();
        let all_vars = all(&env);
        assert_eq!(all_vars.len(), 6);
        assert!(all_vars.contains(&("PORT".to_string(), "3000".to_string())));
        assert!(all_vars.contains(&("DEBUG".to_string(), "true".to_string())));
    }

    #[test]
    fn test_env_cache() {
        let cache = EnvCache::new(mock_env());

        assert_eq!(cache.get("PORT"), Some("3000".to_string()));
        assert_eq!(cache.get_or("PORT", "8080"), "3000");
        assert_eq!(cache.get_or("NONEXISTENT", "default"), "default");
        assert!(cache.bool("DEBUG", false));
        assert!(!cache.bool("NONEXISTENT", false));
        assert_eq!(cache.all().len(), 6);
    }
}
