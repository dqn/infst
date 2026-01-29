//! Song database validation utilities.

use std::collections::HashMap;

use reflux_core::SongInfo;

/// Minimum number of songs expected in the song database
pub const MIN_EXPECTED_SONGS: usize = 1000;
/// Song ID used to verify data readiness (READY FOR TAKEOFF)
pub const READY_SONG_ID: u32 = 80003;
/// Difficulty index to check for note count (SPA)
pub const READY_DIFF_INDEX: usize = 3;
/// Minimum note count expected for the reference song
pub const READY_MIN_NOTES: u32 = 10;

/// Validation result for song database
#[derive(Debug, PartialEq, Eq)]
pub enum ValidationResult {
    Valid,
    TooFewSongs(usize),
    NotecountTooSmall(u32),
    ReferenceSongMissing,
}

/// Validate the song database is fully populated
pub fn validate_song_database(db: &HashMap<u32, SongInfo>) -> ValidationResult {
    if db.len() < MIN_EXPECTED_SONGS {
        return ValidationResult::TooFewSongs(db.len());
    }

    if let Some(song) = db.get(&READY_SONG_ID) {
        let notes = song.total_notes.get(READY_DIFF_INDEX).copied().unwrap_or(0);
        if notes < READY_MIN_NOTES {
            return ValidationResult::NotecountTooSmall(notes);
        }
    } else {
        return ValidationResult::ReferenceSongMissing;
    }

    ValidationResult::Valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use reflux_core::UnlockType;
    use std::sync::Arc;

    fn make_test_song(id: u32, notes: [u32; 10]) -> SongInfo {
        SongInfo {
            id,
            title: Arc::from("Test"),
            title_english: Arc::from("Test"),
            artist: Arc::from("Artist"),
            genre: Arc::from("Genre"),
            bpm: Arc::from("150"),
            folder: 1,
            levels: [0; 10],
            total_notes: notes,
            unlock_type: UnlockType::Base,
        }
    }

    #[test]
    fn test_validate_song_database_valid() {
        let mut db = HashMap::new();
        for i in 1000..2001 {
            db.insert(i, make_test_song(i, [100; 10]));
        }
        db.insert(READY_SONG_ID, make_test_song(READY_SONG_ID, [100; 10]));
        assert_eq!(validate_song_database(&db), ValidationResult::Valid);
    }

    #[test]
    fn test_validate_song_database_too_small() {
        let mut db = HashMap::new();
        for i in 1000..1010 {
            db.insert(i, make_test_song(i, [100; 10]));
        }
        assert_eq!(
            validate_song_database(&db),
            ValidationResult::TooFewSongs(10)
        );
    }

    #[test]
    fn test_validate_song_database_missing_reference_song() {
        let mut db = HashMap::new();
        for i in 1000..2001 {
            db.insert(i, make_test_song(i, [100; 10]));
        }
        // Reference song (80003) is not in the database
        assert_eq!(
            validate_song_database(&db),
            ValidationResult::ReferenceSongMissing
        );
    }

    #[test]
    fn test_validate_song_database_notecount_too_small() {
        let mut db = HashMap::new();
        for i in 1000..2001 {
            db.insert(i, make_test_song(i, [100; 10]));
        }
        // Add reference song with low note count at SPA difficulty
        let mut notes = [100; 10];
        notes[READY_DIFF_INDEX] = 5; // Below READY_MIN_NOTES
        db.insert(READY_SONG_ID, make_test_song(READY_SONG_ID, notes));
        assert_eq!(
            validate_song_database(&db),
            ValidationResult::NotecountTooSmall(5)
        );
    }

    #[test]
    fn test_validate_song_database_empty() {
        let db = HashMap::new();
        assert_eq!(
            validate_song_database(&db),
            ValidationResult::TooFewSongs(0)
        );
    }
}
