//! Debug utilities for analyzing INFINITAS memory structures
//!
//! This module provides tools for:
//! - Checking game and offset status (`StatusInfo`)
//! - Dumping memory structures (`DumpInfo`)
//! - Scanning for song data (`ScanResult`)

mod dump;
mod scan;
mod status;

pub use dump::{DumpInfo, MemoryDump};
pub use scan::{ScanResult, ScannedSong};
pub use status::{OffsetStatus, OffsetValidation, StatusInfo};
