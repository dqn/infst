//! Offset command implementation.

use super::hex_utils::parse_hex_address;
use anyhow::Result;

/// Run the offset command
pub fn run(from: &str, to: &str) -> Result<()> {
    let from_addr = parse_hex_address(from)?;
    let to_addr = parse_hex_address(to)?;

    let diff = to_addr.abs_diff(from_addr);

    let sign = if to_addr >= from_addr { "" } else { "-" };

    println!("From: 0x{:X}", from_addr);
    println!("To:   0x{:X}", to_addr);
    println!();
    println!("Offset: {}{} (0x{:X})", sign, diff, diff);

    Ok(())
}
