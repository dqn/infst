use crate::memory::ReadMemory;
use crate::offset::OffsetsCollection;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Offset dump for diagnostic purposes
#[derive(Debug, Clone, Serialize)]
pub struct OffsetDump {
    pub version: String,
    pub base_address: String,
    pub offsets: OffsetValues,
    pub relations: OffsetRelations,
    pub memory_samples: MemorySamples,
}

/// Offset values in hex string format
#[derive(Debug, Clone, Serialize)]
pub struct OffsetValues {
    pub song_list: String,
    pub unlock_data: String,
    pub data_map: String,
    pub judge_data: String,
    pub play_settings: String,
    pub play_data: String,
    pub current_song: String,
}

/// Relative distances between offsets (signed, in bytes)
#[derive(Debug, Clone, Serialize)]
pub struct OffsetRelations {
    pub song_list_from_base: i64,
    pub unlock_data_from_song_list: i64,
    pub data_map_from_song_list: i64,
    pub judge_data_from_data_map: i64,
    pub play_settings_from_judge_data: i64,
    pub play_data_from_play_settings: i64,
    pub current_song_from_play_settings: i64,
}

/// Memory samples at each offset location
#[derive(Debug, Clone, Serialize)]
pub struct MemorySamples {
    pub play_data_32bytes: String,
    pub current_song_32bytes: String,
    pub judge_data_32bytes: String,
    pub play_settings_32bytes: String,
}

impl OffsetDump {
    /// Create a dump from offsets and memory reader
    pub fn from_offsets<R: ReadMemory>(offsets: &OffsetsCollection, base: u64, reader: &R) -> Self {
        let offset_values = OffsetValues {
            song_list: format!("0x{:X}", offsets.song_list),
            unlock_data: format!("0x{:X}", offsets.unlock_data),
            data_map: format!("0x{:X}", offsets.data_map),
            judge_data: format!("0x{:X}", offsets.judge_data),
            play_settings: format!("0x{:X}", offsets.play_settings),
            play_data: format!("0x{:X}", offsets.play_data),
            current_song: format!("0x{:X}", offsets.current_song),
        };

        let relations = OffsetRelations {
            song_list_from_base: offsets.song_list as i64 - base as i64,
            unlock_data_from_song_list: offsets.unlock_data as i64 - offsets.song_list as i64,
            data_map_from_song_list: offsets.data_map as i64 - offsets.song_list as i64,
            judge_data_from_data_map: offsets.judge_data as i64 - offsets.data_map as i64,
            play_settings_from_judge_data: offsets.play_settings as i64 - offsets.judge_data as i64,
            play_data_from_play_settings: offsets.play_data as i64 - offsets.play_settings as i64,
            current_song_from_play_settings: offsets.current_song as i64
                - offsets.play_settings as i64,
        };

        let memory_samples = MemorySamples {
            play_data_32bytes: Self::read_memory_hex(reader, offsets.play_data, 32),
            current_song_32bytes: Self::read_memory_hex(reader, offsets.current_song, 32),
            judge_data_32bytes: Self::read_memory_hex(reader, offsets.judge_data, 32),
            play_settings_32bytes: Self::read_memory_hex(reader, offsets.play_settings, 32),
        };

        Self {
            version: offsets.version.clone(),
            base_address: format!("0x{:X}", base),
            offsets: offset_values,
            relations,
            memory_samples,
        }
    }

    fn read_memory_hex<R: ReadMemory>(reader: &R, address: u64, size: usize) -> String {
        if address == 0 {
            return "(address is 0)".to_string();
        }

        match reader.read_bytes(address, size) {
            Ok(bytes) => bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" "),
            Err(_) => "(read failed)".to_string(),
        }
    }

    /// Save dump to JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}
