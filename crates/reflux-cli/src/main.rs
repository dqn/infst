mod cli;
mod commands;
mod input;
mod prompter;
mod retry;
mod shutdown;
mod validation;

use anyhow::Result;
use clap::Parser;
use cli::{Args, Command};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging (RUST_LOG がなければ warn を既定にする)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("reflux=warn,reflux_core=warn"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    match args.command {
        Some(Command::FindOffsets { output, pid }) => {
            commands::find_offsets::run(&output, pid)
        }
        Some(Command::Analyze { address, pid }) => {
            commands::analyze::run(address, pid)
        }
        Some(Command::Status { offsets_file, pid, json }) => {
            commands::status::run(offsets_file.as_deref(), pid, json)
        }
        Some(Command::Dump { offsets_file, pid, output }) => {
            commands::dump::run(offsets_file.as_deref(), pid, output.as_deref())
        }
        Some(Command::Scan { offsets_file, pid, range, tsv, output, entry_size }) => {
            commands::scan::run(offsets_file.as_deref(), pid, range, tsv.as_deref(), output.as_deref(), entry_size)
        }
        Some(Command::Explore { address, pid }) => {
            let addr = u64::from_str_radix(address.trim_start_matches("0x").trim_start_matches("0X"), 16)?;
            commands::explore::run(addr, pid)
        }
        Some(Command::Hexdump { address, size, ascii, pid }) => {
            let addr = commands::offset::parse_hex_address(&address)?;
            commands::hexdump::run(addr, size, ascii, pid)
        }
        Some(Command::Search { string, i32, i16, pattern, limit, pid }) => {
            commands::search::run(string, i32, i16, pattern, limit, pid)
        }
        Some(Command::Offset { from, to }) => {
            commands::offset::run(&from, &to)
        }
        Some(Command::Validate { target }) => {
            commands::validate::run(target)
        }
        Some(Command::Export { output, format, pid }) => {
            commands::export::run(output.as_deref(), format, pid)
        }
        None => {
            commands::tracking::run(args.offsets_file.as_deref())
        }
    }
}
