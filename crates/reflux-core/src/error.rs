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

    #[error("Failed to search offset for {target}: {message}")]
    OffsetSearchFailed { target: &'static str, message: String },

    #[error("Invalid game state: expected {expected}, got {actual}")]
    InvalidGameState {
        expected: &'static str,
        actual: String,
    },

    #[error("Song database not loaded: {reason}")]
    SongDatabaseNotLoaded { reason: String },

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

    /// Create an OffsetSearchFailed error with a simple message (for backwards compatibility)
    pub fn offset_search_failed(message: impl Into<String>) -> Self {
        Self::OffsetSearchFailed {
            target: "unknown",
            message: message.into(),
        }
    }

    /// Create an OffsetSearchFailed error with target specification
    pub fn offset_search_failed_for(target: &'static str, message: impl Into<String>) -> Self {
        Self::OffsetSearchFailed {
            target,
            message: message.into(),
        }
    }

    /// Create an InvalidGameState error
    pub fn invalid_game_state(expected: &'static str, actual: impl Into<String>) -> Self {
        Self::InvalidGameState {
            expected,
            actual: actual.into(),
        }
    }

    /// Create a SongDatabaseNotLoaded error
    pub fn song_database_not_loaded(reason: impl Into<String>) -> Self {
        Self::SongDatabaseNotLoaded {
            reason: reason.into(),
        }
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
