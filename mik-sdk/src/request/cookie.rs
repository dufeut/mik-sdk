//! Cookie parsing and building utilities.
//!
//! This module provides:
//! - Parsing of the `Cookie` header from incoming requests
//! - Building `Set-Cookie` headers for responses

/// SameSite attribute for cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SameSite {
    /// Cookie is sent only with same-site requests.
    Strict,
    /// Cookie is sent with same-site requests and top-level navigations.
    Lax,
    /// Cookie is sent with all requests (requires Secure).
    None,
}

impl SameSite {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Lax => "Lax",
            Self::None => "None",
        }
    }
}

/// Builder for `Set-Cookie` header values.
///
/// Creates RFC 6265 compliant Set-Cookie header strings.
///
/// # Example
///
/// ```ignore
/// use mik_sdk::cookie::SetCookie;
///
/// let cookie = SetCookie::new("session", "abc123")
///     .http_only()
///     .secure()
///     .same_site_strict()
///     .max_age(3600)
///     .path("/")
///     .build();
///
/// // Result: "session=abc123; HttpOnly; Secure; SameSite=Strict; Max-Age=3600; Path=/"
/// ```
#[derive(Debug, Clone)]
pub struct SetCookie {
    name: String,
    value: String,
    http_only: bool,
    secure: bool,
    same_site: Option<SameSite>,
    max_age: Option<u64>,
    expires: Option<String>,
    path: Option<String>,
    domain: Option<String>,
}

impl SetCookie {
    /// Create a new Set-Cookie builder with name and value.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            http_only: false,
            secure: false,
            same_site: None,
            max_age: None,
            expires: None,
            path: None,
            domain: None,
        }
    }

    /// Set the HttpOnly flag (cookie not accessible via JavaScript).
    #[must_use]
    pub const fn http_only(mut self) -> Self {
        self.http_only = true;
        self
    }

    /// Set the Secure flag (cookie only sent over HTTPS).
    #[must_use]
    pub const fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    /// Set SameSite=Strict (cookie only sent with same-site requests).
    #[must_use]
    pub const fn same_site_strict(mut self) -> Self {
        self.same_site = Some(SameSite::Strict);
        self
    }

    /// Set SameSite=Lax (cookie sent with same-site requests and top-level navigations).
    #[must_use]
    pub const fn same_site_lax(mut self) -> Self {
        self.same_site = Some(SameSite::Lax);
        self
    }

    /// Set SameSite=None (cookie sent with all requests, requires Secure).
    #[must_use]
    pub const fn same_site_none(mut self) -> Self {
        self.same_site = Some(SameSite::None);
        self
    }

    /// Set Max-Age in seconds (cookie expiration).
    #[must_use]
    pub const fn max_age(mut self, seconds: u64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Set Expires date (RFC 1123 format string).
    ///
    /// Note: Prefer `max_age()` for modern browsers.
    #[must_use]
    pub fn expires(mut self, date: impl Into<String>) -> Self {
        self.expires = Some(date.into());
        self
    }

    /// Set the Path attribute.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set the Domain attribute.
    #[must_use]
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Build the Set-Cookie header value string.
    #[must_use]
    pub fn build(self) -> String {
        let mut parts = vec![format!("{}={}", self.name, self.value)];

        if self.http_only {
            parts.push("HttpOnly".to_string());
        }
        if self.secure {
            parts.push("Secure".to_string());
        }
        if let Some(same_site) = self.same_site {
            parts.push(format!("SameSite={}", same_site.as_str()));
        }
        if let Some(max_age) = self.max_age {
            parts.push(format!("Max-Age={max_age}"));
        }
        if let Some(expires) = self.expires {
            parts.push(format!("Expires={expires}"));
        }
        if let Some(path) = self.path {
            parts.push(format!("Path={path}"));
        }
        if let Some(domain) = self.domain {
            parts.push(format!("Domain={domain}"));
        }

        parts.join("; ")
    }
}

/// Parse a Cookie header value into name-value pairs.
///
/// Cookie header format: `name1=value1; name2=value2; name3=value3`
///
/// Returns borrowed slices from the input string (zero-copy).
/// Invalid pairs (missing `=`) are silently skipped.
pub(super) fn parse_cookie_header(header: &str) -> Vec<(&str, &str)> {
    header
        .split(';')
        .filter_map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return None;
            }
            let (name, value) = pair.split_once('=')?;
            Some((name.trim(), value.trim()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_cookie_basic() {
        let cookie = SetCookie::new("session", "abc123").build();
        assert_eq!(cookie, "session=abc123");
    }

    #[test]
    fn test_set_cookie_http_only() {
        let cookie = SetCookie::new("session", "abc123").http_only().build();
        assert_eq!(cookie, "session=abc123; HttpOnly");
    }

    #[test]
    fn test_set_cookie_secure() {
        let cookie = SetCookie::new("session", "abc123").secure().build();
        assert_eq!(cookie, "session=abc123; Secure");
    }

    #[test]
    fn test_set_cookie_same_site_strict() {
        let cookie = SetCookie::new("session", "abc123")
            .same_site_strict()
            .build();
        assert_eq!(cookie, "session=abc123; SameSite=Strict");
    }

    #[test]
    fn test_set_cookie_same_site_lax() {
        let cookie = SetCookie::new("session", "abc123").same_site_lax().build();
        assert_eq!(cookie, "session=abc123; SameSite=Lax");
    }

    #[test]
    fn test_set_cookie_same_site_none() {
        let cookie = SetCookie::new("session", "abc123")
            .same_site_none()
            .secure()
            .build();
        assert!(cookie.contains("SameSite=None"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn test_set_cookie_max_age() {
        let cookie = SetCookie::new("session", "abc123").max_age(3600).build();
        assert_eq!(cookie, "session=abc123; Max-Age=3600");
    }

    #[test]
    fn test_set_cookie_path() {
        let cookie = SetCookie::new("session", "abc123").path("/api").build();
        assert_eq!(cookie, "session=abc123; Path=/api");
    }

    #[test]
    fn test_set_cookie_domain() {
        let cookie = SetCookie::new("session", "abc123")
            .domain("example.com")
            .build();
        assert_eq!(cookie, "session=abc123; Domain=example.com");
    }

    #[test]
    fn test_set_cookie_full() {
        let cookie = SetCookie::new("session", "abc123")
            .http_only()
            .secure()
            .same_site_strict()
            .max_age(3600)
            .path("/")
            .build();

        assert!(cookie.starts_with("session=abc123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Max-Age=3600"));
        assert!(cookie.contains("Path=/"));
    }

    #[test]
    fn test_parse_cookie_header_basic() {
        let cookies = parse_cookie_header("session=abc123");
        assert_eq!(cookies, vec![("session", "abc123")]);
    }

    #[test]
    fn test_parse_cookie_header_multiple() {
        let cookies = parse_cookie_header("session=abc123; user=john; theme=dark");
        assert_eq!(
            cookies,
            vec![("session", "abc123"), ("user", "john"), ("theme", "dark")]
        );
    }

    #[test]
    fn test_parse_cookie_header_with_spaces() {
        let cookies = parse_cookie_header("  session = abc123 ;  user=john  ");
        assert_eq!(cookies, vec![("session", "abc123"), ("user", "john")]);
    }

    #[test]
    fn test_parse_cookie_header_empty() {
        let cookies = parse_cookie_header("");
        assert!(cookies.is_empty());
    }

    #[test]
    fn test_parse_cookie_header_invalid_pairs() {
        // Invalid pairs (no =) are skipped
        let cookies = parse_cookie_header("valid=value; invalid; also_valid=123");
        assert_eq!(cookies, vec![("valid", "value"), ("also_valid", "123")]);
    }

    #[test]
    fn test_parse_cookie_header_empty_value() {
        let cookies = parse_cookie_header("empty=; normal=value");
        assert_eq!(cookies, vec![("empty", ""), ("normal", "value")]);
    }

    #[test]
    fn test_parse_cookie_header_value_with_equals() {
        // Value can contain = signs (only first = splits name/value)
        let cookies = parse_cookie_header("data=a=b=c");
        assert_eq!(cookies, vec![("data", "a=b=c")]);
    }
}
