//! Memory offset detection and management.
//!
//! This module handles locating game data structures in INFINITAS memory:
//!
//! - **Signature scanning**: Find code patterns to derive data addresses
//! - **Offset collection**: Store and validate detected offsets
//! - **Persistence**: Save/load offsets to files for faster startup
//!
//! ## Architecture
//!
//! The offset detection uses a signature-based approach:
//! 1. Scan game code for known instruction patterns
//! 2. Extract RIP-relative offsets from matching instructions
//! 3. Validate addresses by checking data structure integrity
//!
//! ## Key Types
//!
//! - [`OffsetsCollection`]: All detected memory offsets
//! - [`OffsetSearcher`]: Signature-based offset finder
//! - [`CodeSignature`]: Pattern definition for code scanning

mod collection;
mod dump;
mod loader;
mod searcher;
mod signature;

pub use collection::*;
pub use dump::*;
pub use loader::*;
pub use searcher::*;
pub use signature::*;
