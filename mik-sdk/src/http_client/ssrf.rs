//! SSRF protection utilities for HTTP client.

use super::error::{Error, Result};

/// Check if an authority (host or host:port) refers to a private/internal address.
///
/// Returns `true` if the host is:
/// - `localhost` or `*.localhost`
/// - `127.x.x.x` (loopback)
/// - `10.x.x.x` (private class A)
/// - `172.16.x.x` - `172.31.x.x` (private class B)
/// - `192.168.x.x` (private class C)
/// - `169.254.x.x` (link-local)
/// - `0.0.0.0` (unspecified)
/// - `::1` or `::` (IPv6 loopback/unspecified)
/// - `fe80::` (IPv6 link-local)
/// - `fc00::`/`fd00::` (IPv6 unique local)
pub fn is_private_address(authority: &str) -> bool {
    // Extract host (remove port if present)
    let host = if authority.starts_with('[') {
        // IPv6: [::1]:port or [::1]
        authority
            .find(']')
            .map(|i| &authority[1..i])
            .unwrap_or(authority)
    } else if let Some(colon_idx) = authority.rfind(':') {
        // host:port - but only if the part after colon is all digits
        let potential_port = &authority[colon_idx + 1..];
        if !potential_port.is_empty() && potential_port.chars().all(|c| c.is_ascii_digit()) {
            &authority[..colon_idx]
        } else {
            authority
        }
    } else {
        authority
    };

    let host_lower = host.to_lowercase();

    // Check localhost
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return true;
    }

    // Check IPv4 private ranges
    if let Some((a, rest)) = host.split_once('.')
        && let Ok(first_octet) = a.parse::<u8>()
    {
        // 127.x.x.x (loopback)
        if first_octet == 127 {
            return true;
        }
        // 10.x.x.x (private class A)
        if first_octet == 10 {
            return true;
        }
        // 0.x.x.x (including 0.0.0.0)
        if first_octet == 0 {
            return true;
        }
        // 192.168.x.x (private class C)
        if first_octet == 192
            && let Some((b, _)) = rest.split_once('.')
            && b == "168"
        {
            return true;
        }
        // 172.16-31.x.x (private class B)
        if first_octet == 172
            && let Some((b, _)) = rest.split_once('.')
            && let Ok(second_octet) = b.parse::<u8>()
            && (16..=31).contains(&second_octet)
        {
            return true;
        }
        // 169.254.x.x (link-local)
        if first_octet == 169
            && let Some((b, _)) = rest.split_once('.')
            && b == "254"
        {
            return true;
        }
    }

    // Check IPv6 private addresses (case-insensitive)
    // Remove brackets if present
    let ipv6 = host_lower.trim_start_matches('[').trim_end_matches(']');

    // ::1 (loopback) - can be ::1 or 0:0:0:0:0:0:0:1
    if ipv6 == "::1" || ipv6 == "0:0:0:0:0:0:0:1" {
        return true;
    }
    // :: (unspecified)
    if ipv6 == "::" || ipv6 == "0:0:0:0:0:0:0:0" {
        return true;
    }
    // fe80:: (link-local)
    if ipv6.starts_with("fe80:") || ipv6.starts_with("fe80::") {
        return true;
    }
    // fc00::/fd00:: (unique local addresses)
    if ipv6.starts_with("fc") || ipv6.starts_with("fd") {
        return true;
    }

    false
}

/// Validate the authority component of a URL (host:port).
pub(super) fn validate_authority(authority: &str) -> Result<()> {
    // Check for IPv6 address format: [ipv6]:port or [ipv6]
    if authority.starts_with('[') {
        // IPv6 address
        let close_bracket = authority
            .find(']')
            .ok_or_else(|| Error::InvalidUrl("IPv6 address missing closing bracket".to_string()))?;

        let ipv6_part = &authority[1..close_bracket];
        validate_ipv6(ipv6_part)?;

        // Check for port after the bracket
        let after_bracket = &authority[close_bracket + 1..];
        if !after_bracket.is_empty() {
            if !after_bracket.starts_with(':') {
                return Err(Error::InvalidUrl(
                    "Invalid characters after IPv6 address".to_string(),
                ));
            }
            validate_port(&after_bracket[1..])?;
        }
    } else {
        // Regular host or host:port
        if let Some(colon_idx) = authority.rfind(':') {
            // Could be host:port - check if after colon is a valid port
            let potential_port = &authority[colon_idx + 1..];
            // Only treat as port if it's all digits (avoid IPv6 false positives)
            if !potential_port.is_empty() && potential_port.chars().all(|c| c.is_ascii_digit()) {
                validate_port(potential_port)?;
            }
        }
    }

    Ok(())
}

/// Validate an IPv6 address (without brackets).
fn validate_ipv6(addr: &str) -> Result<()> {
    if addr.is_empty() {
        return Err(Error::InvalidUrl("Empty IPv6 address".to_string()));
    }

    // Count colons and validate segments
    let mut double_colon_count = 0;
    let mut segments = 0;

    for part in addr.split(':') {
        if part.is_empty() {
            double_colon_count += 1;
            continue;
        }

        // Each segment should be 1-4 hex digits
        if part.len() > 4 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::InvalidUrl(format!("Invalid IPv6 segment: {}", part)));
        }
        segments += 1;
    }

    // IPv6 has 8 segments, or fewer with :: compression
    // :: can appear at most once
    if double_colon_count > 3 {
        // More than one "::" sequence (each :: creates 2 empty parts at boundaries)
        return Err(Error::InvalidUrl(
            "Invalid IPv6 address: multiple :: sequences".to_string(),
        ));
    }

    if segments > 8 {
        return Err(Error::InvalidUrl(
            "Invalid IPv6 address: too many segments".to_string(),
        ));
    }

    Ok(())
}

/// Validate a port number.
fn validate_port(port: &str) -> Result<()> {
    if port.is_empty() {
        return Err(Error::InvalidUrl("Empty port number".to_string()));
    }

    // Port must be numeric and within valid range (1-65535)
    let port_num: u32 = port
        .parse()
        .map_err(|_| Error::InvalidUrl(format!("Invalid port number: {}", port)))?;

    if port_num == 0 || port_num > 65535 {
        return Err(Error::InvalidUrl(format!(
            "Port number out of range (1-65535): {}",
            port_num
        )));
    }

    Ok(())
}

/// Validate percent-encoding in a URL path/query.
pub(super) fn validate_percent_encoding(s: &str) -> Result<()> {
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Must have at least 2 more characters
            if i + 2 >= bytes.len() {
                return Err(Error::InvalidUrl(
                    "Incomplete percent-encoding at end of URL".to_string(),
                ));
            }

            // Next two characters must be hex digits
            let hex1 = bytes[i + 1];
            let hex2 = bytes[i + 2];

            if !hex1.is_ascii_hexdigit() || !hex2.is_ascii_hexdigit() {
                return Err(Error::InvalidUrl(format!(
                    "Invalid percent-encoding: %{}{}",
                    char::from(hex1),
                    char::from(hex2)
                )));
            }

            i += 3;
        } else {
            i += 1;
        }
    }

    Ok(())
}
