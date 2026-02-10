//! Search command implementation.

use anyhow::{Result, bail};
use infst::{MemoryReader, ProcessHandle, ReadMemory};

/// Run the search command
pub fn run(
    string: Option<String>,
    i32_val: Option<i32>,
    i16_val: Option<i16>,
    pattern: Option<String>,
    limit: usize,
    pid: Option<u32>,
) -> Result<()> {
    let process = if let Some(pid) = pid {
        ProcessHandle::open(pid)?
    } else {
        ProcessHandle::find_and_open()?
    };

    println!(
        "Found process (PID: {}, Base: 0x{:X})",
        process.pid, process.base_address
    );

    let reader = MemoryReader::new(&process);

    // Determine search pattern
    let (search_bytes, wildcard_mask): (Vec<u8>, Vec<bool>) = if let Some(ref s) = string {
        // Encode string as Shift-JIS
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(s);
        let bytes = encoded.to_vec();
        let mask = vec![false; bytes.len()];
        println!(
            "Searching for string: {:?} ({} bytes, Shift-JIS)",
            s,
            bytes.len()
        );
        (bytes, mask)
    } else if let Some(val) = i32_val {
        let bytes = val.to_le_bytes().to_vec();
        let mask = vec![false; 4];
        println!("Searching for i32: {} (0x{:08X})", val, val as u32);
        (bytes, mask)
    } else if let Some(val) = i16_val {
        let bytes = val.to_le_bytes().to_vec();
        let mask = vec![false; 2];
        println!("Searching for i16: {} (0x{:04X})", val, val as u16);
        (bytes, mask)
    } else if let Some(ref pat) = pattern {
        // Parse byte pattern (e.g., "00 04 07 0A" or "00 ?? 07")
        let parts: Vec<&str> = pat.split_whitespace().collect();
        let mut bytes = Vec::new();
        let mut mask = Vec::new();
        for part in parts {
            if part == "??" {
                bytes.push(0);
                mask.push(true); // wildcard
            } else {
                let byte = u8::from_str_radix(part, 16)
                    .map_err(|_| anyhow::anyhow!("Invalid hex byte: {}", part))?;
                bytes.push(byte);
                mask.push(false);
            }
        }
        println!("Searching for pattern: {} ({} bytes)", pat, bytes.len());
        (bytes, mask)
    } else {
        bail!("No search pattern specified. Use --string, --i32, --i16, or --pattern");
    };

    // Search in memory
    let search_start = process.base_address + 0x1000000; // Start 16MB into the module
    let search_end = process.base_address + (process.module_size as u64).min(0x5000000);
    let chunk_size: usize = 4 * 1024 * 1024; // 4MB chunks

    println!("Search range: 0x{:X} - 0x{:X}", search_start, search_end);
    println!();

    let mut found: Vec<u64> = Vec::new();
    let mut offset = 0u64;

    while search_start + offset < search_end && found.len() < limit {
        let addr = search_start + offset;
        let read_size = chunk_size.min((search_end - addr) as usize);

        if let Ok(buffer) = reader.read_bytes(addr, read_size) {
            for i in 0..=(buffer.len().saturating_sub(search_bytes.len())) {
                let mut matches = true;
                for (j, &byte) in search_bytes.iter().enumerate() {
                    if !wildcard_mask[j] && buffer[i + j] != byte {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    let found_addr = addr + i as u64;
                    found.push(found_addr);

                    println!("[{}] 0x{:X}", found.len(), found_addr);

                    // Show context (32 bytes)
                    if let Ok(context) = reader.read_bytes(found_addr, 32.min(buffer.len() - i)) {
                        print!("     ");
                        for byte in &context[..16.min(context.len())] {
                            print!("{:02X} ", byte);
                        }
                        println!();
                    }

                    if found.len() >= limit {
                        break;
                    }
                }
            }
        }

        offset += chunk_size as u64;
    }

    println!();
    println!("Found {} result(s)", found.len());
    if found.len() >= limit {
        println!("(limit reached, use --limit to increase)");
    }

    Ok(())
}
