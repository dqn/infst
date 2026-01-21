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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Check if this error is a "file not found" error
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::Io(e) if e.kind() == std::io::ErrorKind::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::Io(io_err);
        assert!(err.is_not_found());

        let other_io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err2 = Error::Io(other_io_err);
        assert!(!err2.is_not_found());
    }
}
