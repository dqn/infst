//! Offset cache for faster startup
//!
//! Saves detected offsets to a file and reuses them on subsequent runs,
//! skipping the expensive memory search when the game version matches.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::OffsetsCollection;

/// Cache file name
const CACHE_FILE: &str = ".infst-cache.json";

/// Maximum age for cache validity (24 hours)
const MAX_CACHE_AGE_SECS: u64 = 24 * 60 * 60;

/// Cached offset data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetCache {
    /// Game version string (e.g., "P2D:J:B:A:2026012800")
    pub version: String,
    /// Detected offsets
    pub offsets: OffsetsCollection,
    /// Cache creation timestamp (Unix seconds)
    pub created_at: u64,
}

impl OffsetCache {
    /// Create a new cache entry
    pub fn new(version: String, offsets: OffsetsCollection) -> Self {
        let created_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version,
            offsets,
            created_at,
        }
    }

    /// Load cache from file
    pub fn load() -> Option<Self> {
        Self::load_from_path(CACHE_FILE)
    }

    /// Load cache from a specific path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                debug!("Cache file not found or unreadable: {}", e);
                return None;
            }
        };

        match serde_json::from_str::<OffsetCache>(&content) {
            Ok(cache) => {
                debug!(
                    "Loaded cache: version={}, created_at={}",
                    cache.version, cache.created_at
                );
                Some(cache)
            }
            Err(e) => {
                warn!("Failed to parse cache file: {}", e);
                None
            }
        }
    }

    /// Save cache to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        self.save_to_path(CACHE_FILE)
    }

    /// Save cache to a specific path
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, content)?;
        info!("Saved offset cache to {}", path.as_ref().display());
        Ok(())
    }

    /// Check if cache is valid for the given game version
    pub fn is_valid_for(&self, game_version: &str) -> bool {
        // Check version match
        if self.version != game_version {
            debug!(
                "Cache version mismatch: cached={}, current={}",
                self.version, game_version
            );
            return false;
        }

        // Check cache age
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let age = now.saturating_sub(self.created_at);
        if age > MAX_CACHE_AGE_SECS {
            debug!("Cache expired: age={} seconds", age);
            return false;
        }

        // Check offsets are valid
        if !self.offsets.is_valid() {
            debug!("Cached offsets are invalid (some are zero)");
            return false;
        }

        true
    }
}

/// Try to load cached offsets if valid for the given version
pub fn try_load_cached_offsets(game_version: &str) -> Option<OffsetsCollection> {
    let cache = OffsetCache::load()?;

    if cache.is_valid_for(game_version) {
        info!(
            "Using cached offsets (version: {}, age: {}s)",
            cache.version,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_sub(cache.created_at)
        );
        Some(cache.offsets)
    } else {
        None
    }
}

/// Save offsets to cache
pub fn save_offsets_to_cache(version: &str, offsets: &OffsetsCollection) {
    let cache = OffsetCache::new(version.to_string(), offsets.clone());
    if let Err(e) = cache.save() {
        warn!("Failed to save offset cache: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cache_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let offsets = OffsetsCollection {
            version: "test".to_string(),
            song_list: 0x1000,
            judge_data: 0x2000,
            play_settings: 0x3000,
            play_data: 0x4000,
            current_song: 0x5000,
            data_map: 0x6000,
            unlock_data: 0x7000,
        };

        let cache = OffsetCache::new("P2D:J:B:A:2026012800".to_string(), offsets.clone());
        cache.save_to_path(&path).unwrap();

        let loaded = OffsetCache::load_from_path(&path).unwrap();
        assert_eq!(loaded.version, "P2D:J:B:A:2026012800");
        assert_eq!(loaded.offsets.song_list, 0x1000);
    }

    #[test]
    fn test_cache_version_mismatch() {
        let offsets = OffsetsCollection {
            version: "test".to_string(),
            song_list: 0x1000,
            judge_data: 0x2000,
            play_settings: 0x3000,
            play_data: 0x4000,
            current_song: 0x5000,
            data_map: 0x6000,
            unlock_data: 0x7000,
        };

        let cache = OffsetCache::new("P2D:J:B:A:2026012800".to_string(), offsets);
        assert!(cache.is_valid_for("P2D:J:B:A:2026012800"));
        assert!(!cache.is_valid_for("P2D:J:B:A:2025122400"));
    }

    #[test]
    fn test_cache_invalid_offsets() {
        let offsets = OffsetsCollection::default(); // All zeros
        let cache = OffsetCache::new("P2D:J:B:A:2026012800".to_string(), offsets);
        assert!(!cache.is_valid_for("P2D:J:B:A:2026012800"));
    }
}
