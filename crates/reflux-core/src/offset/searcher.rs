use crate::error::{Error, Result};
use crate::memory::MemoryReader;
use crate::offset::OffsetsCollection;

const INITIAL_SEARCH_SIZE: usize = 2 * 1024 * 1024; // 2MB
#[allow(dead_code)]
const MAX_SEARCH_SIZE: usize = 300 * 1024 * 1024; // 300MB

pub struct OffsetSearcher<'a> {
    reader: &'a MemoryReader<'a>,
    buffer: Vec<u8>,
}

impl<'a> OffsetSearcher<'a> {
    pub fn new(reader: &'a MemoryReader<'a>) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
        }
    }

    pub fn search_all(&mut self) -> Result<OffsetsCollection> {
        let mut offsets = OffsetsCollection::default();

        // Load memory buffer
        self.load_buffer(INITIAL_SEARCH_SIZE)?;

        // Search for version string to find song list offset
        offsets.version = self.search_version()?;
        offsets.song_list = self.search_song_list()?;

        // TODO: Implement other offset searches
        // These require more complex pattern matching based on game state

        Ok(offsets)
    }

    fn load_buffer(&mut self, size: usize) -> Result<()> {
        let base = self.reader.base_address();
        self.buffer = self.reader.read_bytes(base, size)?;
        Ok(())
    }

    fn search_version(&self) -> Result<String> {
        // Search for version pattern "P2D:J:B:A:"
        let pattern = b"P2D:J:B:A:";

        if let Some(pos) = self.find_pattern(pattern) {
            // Read version string (format: P2D:J:B:A:YYMMDDNN)
            let end = self.buffer[pos..]
                .iter()
                .position(|&b| b == 0)
                .map(|p| pos + p)
                .unwrap_or(pos + 30);

            let version_bytes = &self.buffer[pos..end.min(pos + 30)];
            let version = String::from_utf8_lossy(version_bytes).to_string();
            return Ok(version);
        }

        Err(Error::OffsetSearchFailed(
            "Version string not found".to_string(),
        ))
    }

    fn search_song_list(&self) -> Result<u64> {
        // The song list offset is typically near the version string
        // This is a simplified implementation - real implementation needs
        // to follow pointers and validate data structures

        let pattern = b"P2D:J:B:A:";
        if let Some(pos) = self.find_pattern(pattern) {
            // Song list is usually at a known offset from version string
            let base = self.reader.base_address();
            return Ok(base + pos as u64);
        }

        Err(Error::OffsetSearchFailed(
            "Song list offset not found".to_string(),
        ))
    }

    fn find_pattern(&self, pattern: &[u8]) -> Option<usize> {
        self.buffer
            .windows(pattern.len())
            .position(|window| window == pattern)
    }

    #[allow(dead_code)]
    fn find_pattern_with_ignore(&self, pattern: &[u8], ignore_address: usize) -> Option<usize> {
        self.buffer
            .windows(pattern.len())
            .enumerate()
            .find(|(pos, window)| *pos != ignore_address && *window == pattern)
            .map(|(pos, _)| pos)
    }
}

pub fn merge_byte_representations(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_byte_representations() {
        let bytes = merge_byte_representations(&[1, 2]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(bytes[0..4], [1, 0, 0, 0]);
        assert_eq!(bytes[4..8], [2, 0, 0, 0]);
    }
}
