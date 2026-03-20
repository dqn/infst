//! Configuration and support files.
//!
//! This module contains types for configuration and support files:
//! - Version detection utilities
//! - Polling, retry, and database configuration constants

mod version;

pub use version::*;

/// Memory read retry configuration.
///
/// Exponential backoff: 100ms → 200ms → 400ms → 800ms → 1600ms = total ~3.1s max.
/// Longer delays reduce false disconnection detection from transient failures.
pub mod retry {
    /// Maximum number of retry attempts for memory read operations.
    pub const MAX_READ_RETRIES: u32 = 5;

    /// Delay (in ms) for each retry attempt (exponential backoff).
    pub const RETRY_DELAYS_MS: [u64; 5] = [100, 200, 400, 800, 1600];

    // Compile-time check: RETRY_DELAYS_MS must have enough entries for MAX_READ_RETRIES.
    const _: () = assert!(RETRY_DELAYS_MS.len() >= MAX_READ_RETRIES as usize);
}

/// Result screen polling configuration.
///
/// Exponential backoff: 50+50+100+100+200+200+300+300+500+500 = 2.3 seconds max.
/// Faster initial polling catches quick data availability, while exponential
/// backoff reduces CPU usage if data takes longer to populate.
pub mod polling {
    /// Initial delay (in ms) before polling result screen data.
    /// Allows game to finish writing all PlayData fields (especially lamp).
    pub const RESULT_INITIAL_DELAY_MS: u64 = 2000;

    /// Delay (in ms) for each polling attempt on result screen.
    pub const POLL_DELAYS_MS: [u64; 10] = [50, 50, 100, 100, 200, 200, 300, 300, 500, 500];
}

/// Song database loading configuration.
pub mod database {
    use std::time::Duration;

    /// Maximum number of attempts to load the song database.
    pub const MAX_LOAD_ATTEMPTS: u32 = 12;

    /// Maximum number of attempts to search for offsets.
    /// 30 attempts at 5-second intervals = 2.5 minutes.
    pub const MAX_SEARCH_ATTEMPTS: u32 = 30;

    /// Delay between retry attempts.
    pub const RETRY_DELAY: Duration = Duration::from_secs(5);

    /// Extra delay for data initialization.
    pub const EXTRA_DELAY: Duration = Duration::from_secs(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delays_cover_all_attempts() {
        assert!(
            retry::RETRY_DELAYS_MS.len() >= retry::MAX_READ_RETRIES as usize,
            "RETRY_DELAYS_MS must have at least MAX_READ_RETRIES entries"
        );
    }

    #[test]
    fn test_retry_delays_are_non_decreasing() {
        for window in retry::RETRY_DELAYS_MS.windows(2) {
            assert!(
                window[1] >= window[0],
                "retry delays should be non-decreasing: {} < {}",
                window[1],
                window[0]
            );
        }
    }

    #[test]
    fn test_polling_delays_are_positive() {
        assert!(!polling::POLL_DELAYS_MS.is_empty());
        for &delay in &polling::POLL_DELAYS_MS {
            assert!(delay > 0, "polling delays must be positive");
        }
    }

    #[test]
    fn test_database_retry_delay_is_positive() {
        assert!(!database::RETRY_DELAY.is_zero());
    }

    #[test]
    fn test_database_search_attempts_greater_than_load_attempts() {
        assert!(
            database::MAX_SEARCH_ATTEMPTS > database::MAX_LOAD_ATTEMPTS,
            "search should retry more times than load since it waits for game initialization"
        );
    }
}
