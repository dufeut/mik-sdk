//! Cryptographically secure random value generation.
//!
//! This module provides convenient functions for generating random values
//! using the runtime's cryptographic random number generator.
//!
//! # Platform Support
//!
//! | Platform | Implementation | Notes |
//! |----------|----------------|-------|
//! | **WASM** | `wasi:random/random` | Production - uses runtime's secure RNG |
//! | **Native (tests)** | `getrandom` crate | Testing only - uses OS entropy |
//! | **Native (non-test)** | Compile stub | Panics at runtime with helpful message |
//!
//! # Why WASM-only?
//!
//! This SDK is designed for WASI HTTP handlers that run on any WASI P2 runtime.
//! The random functions use `wasi:random/random` for cryptographic randomness in WASM.
//!
//! Native builds compile (so `cargo check` works) but panic at runtime with a clear
//! error message directing you to compile for `wasm32-wasip2`.
//!
//! # Security
//!
//! All functions in this module use cryptographically secure randomness
//! suitable for generating tokens, secrets, and UUIDs.
//!
//! # Examples
//!
//! ```ignore
//! use mik_sdk::random;
//!
//! // Generate random bytes
//! let secret = random::bytes(32);
//! assert_eq!(secret.len(), 32);
//!
//! // Generate a random u64
//! let id = random::u64();
//!
//! // Generate a UUID v4
//! let uuid = random::uuid();
//! assert_eq!(uuid.len(), 36); // "550e8400-e29b-41d4-a716-446655440000"
//!
//! // Generate random hex string
//! let token = random::hex(16);
//! assert_eq!(token.len(), 32); // 16 bytes = 32 hex characters
//! ```

// =============================================================================
// WASM IMPLEMENTATION (Production)
// Uses wasi:random/random for cryptographic randomness
// =============================================================================

/// Generate cryptographically secure random bytes.
///
/// Uses the runtime's secure random number generator:
/// - WASM: `wasi:random/random` directly
/// - Native (tests): OS entropy via getrandom crate
///
/// # Panics
///
/// Panics if the underlying RNG fails (extremely rare, indicates critical system issue).
/// On native non-test builds, always panics with a message to use WASM target.
///
/// # Examples
///
/// ```ignore
/// let key = mik_sdk::random::bytes(32);
/// assert_eq!(key.len(), 32);
/// ```
#[must_use]
#[cfg(target_arch = "wasm32")]
pub fn bytes(len: usize) -> Vec<u8> {
    crate::wasi_http::wasi::random::random::get_random_bytes(len as u64)
}

// =============================================================================
// NATIVE TEST IMPLEMENTATION
// Uses getrandom crate for OS entropy (dev-dependency)
// =============================================================================

/// Generate cryptographically secure random bytes (native test implementation).
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), test))]
pub fn bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    getrandom::fill(&mut buf).expect("system RNG failure");
    buf
}

// =============================================================================
// NATIVE NON-TEST STUBS
// These compile but panic at runtime - allows cargo check/build to work
// =============================================================================

/// Stub for native non-test builds. Panics with helpful error message.
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
pub fn bytes(_len: usize) -> Vec<u8> {
    panic!(
        "\n\
        ╔══════════════════════════════════════════════════════════════════╗\n\
        ║  random::bytes() is only available in WASM builds                ║\n\
        ╠══════════════════════════════════════════════════════════════════╣\n\
        ║                                                                  ║\n\
        ║  This SDK is designed for WASI HTTP handlers.                    ║\n\
        ║  Random functions use wasi:random/random in WASM.                ║\n\
        ║                                                                  ║\n\
        ║  To build for WASM:                                              ║\n\
        ║    cargo component build --target wasm32-wasip2                  ║\n\
        ║                                                                  ║\n\
        ║  Or run tests (which use getrandom):                             ║\n\
        ║    cargo test                                                    ║\n\
        ║                                                                  ║\n\
        ╚══════════════════════════════════════════════════════════════════╝\n"
    )
}

/// Generate a cryptographically secure random u64.
///
/// # Panics
///
/// Panics if the underlying RNG fails (extremely rare, indicates critical system issue).
/// On native non-test builds, always panics with a message to use WASM target.
///
/// # Examples
///
/// ```ignore
/// let id = mik_sdk::random::u64();
/// ```
#[must_use]
#[cfg(target_arch = "wasm32")]
pub fn u64() -> u64 {
    crate::wasi_http::wasi::random::random::get_random_u64()
}

/// Generate a cryptographically secure random u64 (native test implementation).
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), test))]
pub fn u64() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("system RNG failure");
    u64::from_le_bytes(buf)
}

/// Stub for native non-test builds. Panics with helpful error message.
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
pub fn u64() -> u64 {
    panic!(
        "\n\
        ╔══════════════════════════════════════════════════════════════════╗\n\
        ║  random::u64() is only available in WASM builds                  ║\n\
        ╠══════════════════════════════════════════════════════════════════╣\n\
        ║                                                                  ║\n\
        ║  This SDK is designed for WASI HTTP handlers.                    ║\n\
        ║  Random functions use wasi:random/random in WASM.                ║\n\
        ║                                                                  ║\n\
        ║  To build for WASM:                                              ║\n\
        ║    cargo component build --target wasm32-wasip2                  ║\n\
        ║                                                                  ║\n\
        ╚══════════════════════════════════════════════════════════════════╝\n"
    )
}

/// Generate a UUID v4 string.
///
/// Returns a standard UUID v4 format: `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`
/// where `x` is any hexadecimal digit and `y` is one of `8`, `9`, `a`, or `b`.
///
/// # Panics
///
/// Panics if the underlying RNG fails (extremely rare, indicates critical system issue).
/// On native non-test builds, always panics with a message to use WASM target.
///
/// # Examples
///
/// ```ignore
/// let id = mik_sdk::random::uuid();
/// assert_eq!(id.len(), 36);
/// assert_eq!(id.chars().nth(14), Some('4')); // Version 4
/// ```
#[must_use]
#[cfg(any(target_arch = "wasm32", test))]
pub fn uuid() -> String {
    let mut buf = bytes(16);

    // Set version bits (4 bits at byte 6, high nibble = 0100 for v4)
    buf[6] = (buf[6] & 0x0F) | 0x40;

    // Set variant bits (2 bits at byte 8, high 2 bits = 10 for RFC 4122)
    buf[8] = (buf[8] & 0x3F) | 0x80;

    // Format as UUID string
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        buf[0],
        buf[1],
        buf[2],
        buf[3],
        buf[4],
        buf[5],
        buf[6],
        buf[7],
        buf[8],
        buf[9],
        buf[10],
        buf[11],
        buf[12],
        buf[13],
        buf[14],
        buf[15]
    )
}

/// Stub for native non-test builds. Panics with helpful error message.
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
pub fn uuid() -> String {
    panic!(
        "\n\
        ╔══════════════════════════════════════════════════════════════════╗\n\
        ║  random::uuid() is only available in WASM builds                 ║\n\
        ╠══════════════════════════════════════════════════════════════════╣\n\
        ║                                                                  ║\n\
        ║  This SDK is designed for WASI HTTP handlers.                    ║\n\
        ║  Random functions use wasi:random/random in WASM.                ║\n\
        ║                                                                  ║\n\
        ║  To build for WASM:                                              ║\n\
        ║    cargo component build --target wasm32-wasip2                  ║\n\
        ║                                                                  ║\n\
        ╚══════════════════════════════════════════════════════════════════╝\n"
    )
}

/// Generate a random hexadecimal string.
///
/// # Panics
///
/// Panics if the underlying RNG fails (extremely rare, indicates critical system issue).
/// On native non-test builds, always panics with a message to use WASM target.
///
/// # Examples
///
/// ```ignore
/// let token = mik_sdk::random::hex(16);
/// assert_eq!(token.len(), 32); // 16 bytes = 32 hex chars
/// ```
#[must_use]
#[cfg(any(target_arch = "wasm32", test))]
pub fn hex(byte_len: usize) -> String {
    use crate::constants::HEX_CHARS;
    let random_bytes = bytes(byte_len);
    // Pre-allocate exact capacity: 2 hex chars per byte
    let mut result = String::with_capacity(byte_len * 2);
    for b in random_bytes {
        result.push(HEX_CHARS[(b >> 4) as usize] as char);
        result.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    result
}

/// Stub for native non-test builds. Panics with helpful error message.
#[must_use]
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
pub fn hex(_byte_len: usize) -> String {
    panic!(
        "\n\
        ╔══════════════════════════════════════════════════════════════════╗\n\
        ║  random::hex() is only available in WASM builds                  ║\n\
        ╠══════════════════════════════════════════════════════════════════╣\n\
        ║                                                                  ║\n\
        ║  This SDK is designed for WASI HTTP handlers.                    ║\n\
        ║  Random functions use wasi:random/random in WASM.                ║\n\
        ║                                                                  ║\n\
        ║  To build for WASM:                                              ║\n\
        ║    cargo component build --target wasm32-wasip2                  ║\n\
        ║                                                                  ║\n\
        ╚══════════════════════════════════════════════════════════════════╝\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_length() {
        assert_eq!(bytes(16).len(), 16);
        assert_eq!(bytes(32).len(), 32);
        assert_eq!(bytes(0).len(), 0);
    }

    #[test]
    fn test_bytes_randomness() {
        // Multiple calls should produce different results
        let b1 = bytes(32);
        let b2 = bytes(32);
        assert_ne!(b1, b2, "Two random byte arrays should differ");
    }

    #[test]
    fn test_u64_returns_value() {
        let v = u64();
        // Just verify it returns something (randomness means any value is valid)
        // We can't test for a specific value, but we can test the function runs
        let _ = v;
    }

    #[test]
    fn test_u64_randomness() {
        // Multiple calls should produce different results
        let v1 = u64();
        let v2 = u64();
        // Note: theoretically could be same, but astronomically unlikely
        assert_ne!(v1, v2, "Two random u64s should differ");
    }

    #[test]
    fn test_uuid_format() {
        let id = uuid();
        assert_eq!(id.len(), 36);

        // Check hyphens
        assert_eq!(id.chars().nth(8), Some('-'));
        assert_eq!(id.chars().nth(13), Some('-'));
        assert_eq!(id.chars().nth(18), Some('-'));
        assert_eq!(id.chars().nth(23), Some('-'));

        // Check version is 4
        assert_eq!(id.chars().nth(14), Some('4'));

        // Check variant (8, 9, a, or b)
        let variant = id
            .chars()
            .nth(19)
            .expect("UUID must have character at position 19");
        assert!(matches!(variant, '8' | '9' | 'a' | 'b'));
    }

    #[test]
    fn test_uuid_version_bits() {
        // Generate many UUIDs and verify version bits are always correct
        for _ in 0..100 {
            let id = uuid();
            // Version should always be 4
            assert_eq!(id.chars().nth(14), Some('4'));
        }
    }

    #[test]
    fn test_uuid_variant_bits() {
        // Generate many UUIDs and verify variant bits are always correct
        for _ in 0..100 {
            let id = uuid();
            let variant = id.chars().nth(19).unwrap();
            assert!(
                matches!(variant, '8' | '9' | 'a' | 'b'),
                "Variant char '{variant}' not in valid set [8,9,a,b]"
            );
        }
    }

    #[test]
    fn test_uuid_version_byte_exact() {
        // Verify byte 6 has correct version bits (0x40 OR'd, 0x0F AND'd)
        // The 7th and 8th hex chars represent byte 6
        for _ in 0..100 {
            let id = uuid();
            // Extract the version byte (positions 14-15 in the string, after removing hyphens)
            // Format: xxxxxxxx-xxxx-Vxxx-yxxx-xxxxxxxxxxxx
            // Position 14 is 'V' (version nibble), should always be '4'
            let version_char = id.chars().nth(14).unwrap();
            assert_eq!(
                version_char, '4',
                "Version nibble must be 4, got {version_char}"
            );

            // The high nibble of byte 6 must be exactly 0x4 (binary 0100)
            // If mutation changed | to ^, with random input 0x4X, XOR would give wrong results
            let byte6_high = u8::from_str_radix(&id[14..15], 16).unwrap();
            assert_eq!(byte6_high, 4, "Version high nibble must be 0x4");
        }
    }

    #[test]
    fn test_uuid_variant_byte_exact() {
        // Verify byte 8 has correct variant bits (0x80 OR'd, 0x3F AND'd)
        // The variant byte is at position 19 in the UUID string
        // Format: xxxxxxxx-xxxx-4xxx-Yxxx-xxxxxxxxxxxx
        // Y must be 8, 9, a, or b (binary 10xx)
        for _ in 0..100 {
            let id = uuid();
            let variant_char = id.chars().nth(19).unwrap();
            let variant_nibble = u8::from_str_radix(&variant_char.to_string(), 16).unwrap();

            // The high 2 bits must be 10 (binary), meaning value is 8-11 (0x8-0xB)
            assert!(
                (8..=11).contains(&variant_nibble),
                "Variant nibble must be 8-11 (0x8-0xB), got {variant_nibble}"
            );

            // If | was mutated to ^, the high bit pattern would be wrong
            // With 0x80 OR, we force bit 7 to 1. With XOR on random, it could be 0.
            // With 0x3F AND first, we clear bits 6-7, then OR sets bit 7.
            // The result must have bit 7 set (value >= 8) and bit 6 clear (value <= 11)
            assert!((variant_nibble & 0x8) != 0, "Variant bit 7 must be set");
            assert!((variant_nibble & 0x4) == 0, "Variant bit 6 must be clear");
        }
    }

    #[test]
    fn test_uuid_uniqueness() {
        let id1 = uuid();
        let id2 = uuid();
        assert_ne!(id1, id2, "Two UUIDs should be unique");
    }

    #[test]
    fn test_uuid_all_lowercase() {
        let id = uuid();
        let hex_chars: String = id.chars().filter(|c| *c != '-').collect();
        assert!(
            hex_chars
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "UUID should be all lowercase hex"
        );
    }

    #[test]
    fn test_uuid_raw_bytes_version() {
        // Parse UUID string back to bytes and verify exact bit patterns
        // This catches mutations to AND/OR operations that string tests might miss
        for _ in 0..100 {
            let id = uuid();
            // Parse UUID string back to bytes
            let hex_str: String = id.chars().filter(|c| *c != '-').collect();
            let bytes: Vec<u8> = (0..16)
                .map(|i| u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16).unwrap())
                .collect();

            // Verify version byte (byte 6): high nibble must be exactly 0x40
            // The operation is: (buf[6] & 0x0F) | 0x40
            // - AND 0x0F clears high nibble
            // - OR 0x40 sets exactly bit 6 (value 0x40)
            // Result: high nibble MUST be 0x4, low nibble is random
            assert_eq!(
                bytes[6] & 0xF0,
                0x40,
                "Version byte high nibble must be 0x40, got {:#04x}",
                bytes[6]
            );
        }
    }

    #[test]
    fn test_uuid_raw_bytes_variant() {
        // Parse UUID string back to bytes and verify variant bit pattern
        for _ in 0..100 {
            let id = uuid();
            let hex_str: String = id.chars().filter(|c| *c != '-').collect();
            let bytes: Vec<u8> = (0..16)
                .map(|i| u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16).unwrap())
                .collect();

            // Verify variant byte (byte 8): high 2 bits must be 10
            // The operation is: (buf[8] & 0x3F) | 0x80
            // - AND 0x3F clears bits 6-7
            // - OR 0x80 sets bit 7
            // Result: bits 7-6 MUST be 10 (binary), meaning 0x80-0xBF range
            assert_eq!(
                bytes[8] & 0xC0,
                0x80,
                "Variant byte high 2 bits must be 10, got {:#04x}",
                bytes[8]
            );
        }
    }

    #[test]
    fn test_hex_length() {
        assert_eq!(hex(8).len(), 16);
        assert_eq!(hex(16).len(), 32);
    }

    #[test]
    fn test_hex_zero_length() {
        assert_eq!(hex(0).len(), 0);
        assert_eq!(hex(0), "");
    }

    #[test]
    fn test_hex_is_valid() {
        let h = hex(16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(
            h.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn test_hex_uniqueness() {
        let h1 = hex(16);
        let h2 = hex(16);
        assert_ne!(h1, h2, "Two hex strings should differ");
    }
}
