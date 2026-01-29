//! Offset command implementation.

use anyhow::Result;

/// Parse a hex address string (with or without 0x prefix)
pub fn parse_hex_address(s: &str) -> Result<u64> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!("Invalid hex address: {}", e))
}

/// Run the offset command
pub fn run(from: &str, to: &str) -> Result<()> {
    let from_addr = parse_hex_address(from)?;
    let to_addr = parse_hex_address(to)?;

    let diff = if to_addr >= from_addr {
        to_addr - from_addr
    } else {
        from_addr - to_addr
    };

    let sign = if to_addr >= from_addr { "" } else { "-" };

    println!("From: 0x{:X}", from_addr);
    println!("To:   0x{:X}", to_addr);
    println!();
    println!("Offset: {}{} (0x{:X})", sign, diff, diff);

    Ok(())
}
