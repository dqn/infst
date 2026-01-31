//! Hexdump command implementation.
//!
//! Displays raw memory bytes in traditional hexdump format, useful for
//! investigating memory structures and debugging offset calculations.
//!
//! # Output Format
//!
//! ```text
//! 0x000: 48 65 6C 6C 6F 20 57 6F  72 6C 64 00 00 00 00 00  |Hello World.....|
//! ```

use anyhow::Result;
use reflux_core::{MemoryReader, ProcessHandle, ReadMemory};

/// Run the hexdump command
pub fn run(address: u64, size: usize, ascii: bool, pid: Option<u32>) -> Result<()> {
    let process = if let Some(pid) = pid {
        ProcessHandle::open(pid)?
    } else {
        ProcessHandle::find_and_open()?
    };

    let reader = MemoryReader::new(&process);
    let bytes = reader.read_bytes(address, size)?;

    println!("Hexdump at 0x{:X} ({} bytes):", address, size);
    println!();

    for (i, chunk) in bytes.chunks(16).enumerate() {
        let offset = i * 16;
        print!("0x{:03X}: ", offset);

        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            if j == 8 {
                print!(" ");
            }
            print!("{:02X} ", byte);
        }

        // Padding for incomplete lines
        if chunk.len() < 16 {
            for j in chunk.len()..16 {
                if j == 8 {
                    print!(" ");
                }
                print!("   ");
            }
        }

        // ASCII representation
        if ascii {
            print!(" |");
            for byte in chunk {
                if *byte >= 0x20 && *byte < 0x7F {
                    print!("{}", *byte as char);
                } else {
                    print!(".");
                }
            }
            for _ in chunk.len()..16 {
                print!(" ");
            }
            print!("|");
        }

        println!();
    }

    Ok(())
}
