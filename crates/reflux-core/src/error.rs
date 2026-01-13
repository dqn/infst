use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Process not found: {0}")]
    ProcessNotFound(String),

    #[error("Failed to open process: {0}")]
    ProcessOpenFailed(String),

    #[error("Failed to read process memory at address {address:#x}: {message}")]
    MemoryReadFailed { address: u64, message: String },

    #[error("Invalid offset: {0}")]
    InvalidOffset(String),

    #[error("Offset version mismatch: expected {expected}, got {actual}")]
    OffsetVersionMismatch { expected: String, actual: String },

    #[error("Failed to search offset: {0}")]
    OffsetSearchFailed(String),

    #[error("Invalid game state")]
    InvalidGameState,

    #[error("Song database not loaded")]
    SongDatabaseNotLoaded,

    #[error("Config parse error: {0}")]
    ConfigParseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

pub type Result<T> = std::result::Result<T, Error>;
