//! Hex address parsing and formatting utilities.

use anyhow::Result;

/// Parse a hex address string (with or without 0x prefix).
///
/// # Examples
///
/// ```
/// use reflux::commands::hex_utils::parse_hex_address;
///
/// assert_eq!(parse_hex_address("0x1000").unwrap(), 0x1000);
/// assert_eq!(parse_hex_address("1000").unwrap(), 0x1000);
/// assert_eq!(parse_hex_address("0X1000").unwrap(), 0x1000);
/// ```
pub fn parse_hex_address(s: &str) -> Result<u64> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!("Invalid hex address: {}", e))
}

/// Format an address as a hex string with 0x prefix.
///
/// # Examples
///
/// ```
/// use reflux::commands::hex_utils::format_hex_address;
///
/// assert_eq!(format_hex_address(0x1000), "0x1000");
/// ```
#[allow(dead_code)]
pub fn format_hex_address(addr: u64) -> String {
    format!("0x{:X}", addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_address_with_prefix() {
        assert_eq!(parse_hex_address("0x1000").unwrap(), 0x1000);
        assert_eq!(parse_hex_address("0X1000").unwrap(), 0x1000);
    }

    #[test]
    fn test_parse_hex_address_without_prefix() {
        assert_eq!(parse_hex_address("1000").unwrap(), 0x1000);
        assert_eq!(parse_hex_address("DEADBEEF").unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_parse_hex_address_large() {
        assert_eq!(parse_hex_address("0x1431B08A0").unwrap(), 0x1431B08A0);
    }

    #[test]
    fn test_parse_hex_address_invalid() {
        assert!(parse_hex_address("GHIJK").is_err());
        assert!(parse_hex_address("0xZZZ").is_err());
    }

    #[test]
    fn test_format_hex_address() {
        assert_eq!(format_hex_address(0x1000), "0x1000");
        assert_eq!(format_hex_address(0xDEADBEEF), "0xDEADBEEF");
        assert_eq!(format_hex_address(0), "0x0");
    }
}
