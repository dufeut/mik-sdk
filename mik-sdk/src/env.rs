//! Environment variable access for WASI HTTP handlers.
//!
//! This module provides ergonomic wrappers around `wasi:cli/environment` for
//! accessing environment variables in handler code.
//!
//! # Usage
//!
//! In your handler component (which has access to WASI bindings):
//!
//! ```ignore
//! use bindings::wasi::cli::environment;
//! use mik_sdk::env;
//!
//! fn my_handler(req: &Request) -> Response {
//!     // Get environment variables
//!     let port = env::get_or(&environment::get_environment(), "PORT", "8080");
//!     let debug = env::bool(&environment::get_environment(), "DEBUG", false);
//!
//!     ok!({ "port": str(port), "debug": bool(debug) })
//! }
//! ```
//!
//! # Caching
//!
//! For better performance, cache the environment on first access:
//!
//! ```ignore
//! use std::sync::OnceLock;
//! use bindings::wasi::cli::environment;
//!
//! static ENV: OnceLock<Vec<(String, String)>> = OnceLock::new();
//!
//! fn env() -> &'static Vec<(String, String)> {
//!     ENV.get_or_init(|| environment::get_environment())
//! }
//!
//! fn my_handler(req: &Request) -> Response {
//!     let port = env::get_or(env(), "PORT", "8080");
//!     ok!({ "port": str(port) })
//! }
//! ```

use std::collections::HashMap;

/// Get an environment variable by name.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::cli::environment;
/// use mik_sdk::env;
///
/// let db_url = env::get(&environment::get_environment(), "DATABASE_URL");
/// if let Some(url) = db_url {
///     // Connect to database
/// }
/// ```
#[must_use]
pub fn get(env: &[(String, String)], name: &str) -> Option<String> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// Get an environment variable or return a default value.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::cli::environment;
/// use mik_sdk::env;
///
/// let port = env::get_or(&environment::get_environment(), "PORT", "8080");
/// let host = env::get_or(&environment::get_environment(), "HOST", "0.0.0.0");
/// ```
#[must_use]
pub fn get_or(env: &[(String, String)], name: &str, default: &str) -> String {
    get(env, name).unwrap_or_else(|| default.to_string())
}

/// Get an environment variable as a boolean.
///
/// Returns `true` if the value is "true", "1", or "yes" (case-insensitive).
/// Returns `false` if the variable is not set or has any other value.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::cli::environment;
/// use mik_sdk::env;
///
/// let debug = env::bool(&environment::get_environment(), "DEBUG", false);
/// let verbose = env::bool(&environment::get_environment(), "VERBOSE", false);
///
/// if debug {
///     // Enable debug logging
/// }
/// ```
#[must_use]
pub fn bool(env: &[(String, String)], name: &str, default: bool) -> bool {
    get(env, name).map_or(default, |v| {
        let v_lower = v.to_lowercase();
        v_lower == "true" || v_lower == "1" || v_lower == "yes"
    })
}

/// Get all environment variables.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::cli::environment;
/// use mik_sdk::env;
///
/// let all = env::all(&environment::get_environment());
/// for (key, value) in all {
///     println!("{key}={value}");
/// }
/// ```
#[must_use]
pub fn all(env: &[(String, String)]) -> Vec<(String, String)> {
    env.to_vec()
}

/// Environment variable cache for efficient repeated access.
///
/// Uses a `HashMap` for O(1) lookups instead of linear search.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::cli::environment;
/// use mik_sdk::env::EnvCache;
///
/// // Initialize once at module level
/// static ENV_CACHE: std::sync::OnceLock<EnvCache> = std::sync::OnceLock::new();
///
/// fn env_cache() -> &'static EnvCache {
///     ENV_CACHE.get_or_init(|| EnvCache::new(environment::get_environment()))
/// }
///
/// fn my_handler(req: &Request) -> Response {
///     let port = env_cache().get_or("PORT", "8080");
///     let debug = env_cache().bool("DEBUG", false);
///     ok!({ "port": str(port), "debug": bool(debug) })
/// }
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
