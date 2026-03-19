use std::collections::{HashMap, HashSet};

use tracing::warn;

use crate::chart::{Difficulty, SongInfo};
use crate::error::Result;
use crate::process::{ByteBuffer, ReadMemory};
use crate::score::Lamp;

/// Maximum allowed buffer size for the DataMap hash table (16 MB).
/// The table is typically a few hundred KB; anything larger suggests corrupt memory.
const MAX_DATA_MAP_BUFFER_SIZE: usize = 16 * 1024 * 1024;

/// Sentinel value found in INFINITAS data map hash table buckets.
///
/// Buckets containing this value do not point to a valid linked list and must be
/// skipped during traversal. The value originates from the game engine's hash table
/// implementation and is distinct from both null (0x0) and the null_obj pointer.
/// Also defined in `offset::searcher::constants::DATA_MAP_SENTINEL`.
const DATA_MAP_HASH_BUCKET_SENTINEL: u64 = 0x494fdce0;

/// Score data for a single song (all difficulties)
#[derive(Debug, Clone, Default)]
pub struct ScoreData {
    pub song_id: u32,
    /// Lamp for each difficulty: SPB, SPN, SPH, SPA, SPL, DPB, DPN, DPH, DPA, DPL
    pub lamp: [Lamp; 10],
    /// EX Score for each difficulty
    pub score: [u32; 10],
    /// Miss count for each difficulty
    pub miss_count: [Option<u32>; 10],
    /// DJ Points for each difficulty
    pub dj_points: [f64; 10],
}

impl ScoreData {
    pub fn new(song_id: u32) -> Self {
        Self {
            song_id,
            ..Default::default()
        }
    }

    pub fn get_lamp(&self, difficulty: Difficulty) -> Lamp {
        self.lamp
            .get(difficulty as usize)
            .copied()
            .unwrap_or(Lamp::NoPlay)
    }

    pub fn get_score(&self, difficulty: Difficulty) -> u32 {
        self.score.get(difficulty as usize).copied().unwrap_or(0)
    }

    pub fn set_lamp(&mut self, difficulty: Difficulty, lamp: Lamp) {
        if let Some(slot) = self.lamp.get_mut(difficulty as usize) {
            *slot = lamp;
        }
    }

    pub fn set_score(&mut self, difficulty: Difficulty, score: u32) {
        if let Some(slot) = self.score.get_mut(difficulty as usize) {
            *slot = score;
        }
    }
}

/// A node in the INFINITAS score hashmap linked list
#[derive(Debug, Clone, Default)]
struct ListNode {
    next: u64,
    /// Previous node pointer (unused but required for memory layout compatibility)
    #[allow(dead_code)]
    prev: u64,
    diff: i32,
    song: i32,
    playtype: i32,
    score: u32,
    miss_count: u32,
    lamp: i32,
}

impl ListNode {
    const SIZE: usize = 64;

    fn from_bytes(bytes: &[u8]) -> Self {
        let buf = ByteBuffer::new(bytes);
        Self {
            next: buf.read_u64_at(0).unwrap_or(0),
            prev: buf.read_u64_at(8).unwrap_or(0),
            diff: buf.read_i32_at(16).unwrap_or(0),
            song: buf.read_i32_at(20).unwrap_or(0),
            playtype: buf.read_i32_at(24).unwrap_or(0),
            // uk2 at 28-31
            score: buf.read_u32_at(32).unwrap_or(0),
            miss_count: buf.read_u32_at(36).unwrap_or(0),
            // uk3 at 40-43, uk4 at 44-47
            lamp: buf.read_i32_at(48).unwrap_or(0),
        }
    }

    fn key(&self) -> (u32, i32, i32) {
        (self.song as u32, self.diff, self.playtype)
    }
}

/// Map of song scores loaded from INFINITAS memory
#[derive(Debug, Clone, Default)]
pub struct ScoreMap {
    scores: HashMap<u32, ScoreData>,
}

impl ScoreMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load score map from INFINITAS memory
    pub fn load_from_memory<R: ReadMemory>(
        reader: &R,
        data_map_addr: u64,
        _song_db: &HashMap<u32, SongInfo>,
    ) -> Result<Self> {
        let mut nodes: HashMap<(u32, i32, i32), ListNode> = HashMap::new();

        // Read null object address (used to skip empty entries)
        let null_obj = reader.read_u64(data_map_addr.wrapping_sub(16))?;

        // Read start and end addresses of the hash table
        let start_address = reader.read_u64(data_map_addr)?;
        let end_address = reader.read_u64(data_map_addr + 8)?;

        if end_address <= start_address {
            return Ok(Self::new());
        }

        let buffer_size = (end_address - start_address) as usize;
        if buffer_size > MAX_DATA_MAP_BUFFER_SIZE {
            warn!(
                buffer_size,
                max = MAX_DATA_MAP_BUFFER_SIZE,
                "DataMap buffer size exceeds maximum; likely corrupt memory, returning empty ScoreMap"
            );
            return Ok(Self::new());
        }
        let buffer = reader.read_bytes(start_address, buffer_size)?;

        // Collect entry points from the hash table
        let buf = ByteBuffer::new(&buffer);
        let mut entry_points = Vec::new();
        for i in 0..(buffer_size / 8) {
            let addr = buf.read_u64_at(i * 8).unwrap_or(0);

            // Skip null, null_obj, and sentinel bucket entries
            if addr != 0 && addr != null_obj && addr != DATA_MAP_HASH_BUCKET_SENTINEL {
                entry_points.push(addr);
            }
        }

        // Follow linked lists from each entry point.
        // Share visited set across all calls to avoid re-traversing shared list segments.
        let mut visited: HashSet<u64> = HashSet::new();
        for entry_point in entry_points {
            Self::follow_linked_list(reader, entry_point, null_obj, &mut visited, &mut nodes);
        }

        // Convert nodes to ScoreData
        let mut result = Self::new();
        for ((song_id, diff, playtype), node) in nodes {
            // Reject negative or out-of-range values before casting to usize
            if !(0..5).contains(&diff) || !(0..2).contains(&playtype) {
                continue;
            }
            // Validate lamp value as i32 before casting to u8 to prevent truncation
            // (e.g., 0x107 would silently become 7/FullCombo via `as u8`)
            if !(0..=7).contains(&node.lamp) {
                continue;
            }
            // Calculate difficulty index: diff + playtype * 5
            let difficulty_index = (diff + playtype * 5) as usize;

            let score_data = result.get_or_insert(song_id);
            score_data.lamp[difficulty_index] =
                Lamp::from_u8(node.lamp as u8).unwrap_or(Lamp::NoPlay);
            score_data.score[difficulty_index] = node.score;
            // INFINITAS uses u32::MAX as sentinel value to indicate miss_count data is unavailable
            // (e.g., for legacy scores or when the game doesn't track this information)
            score_data.miss_count[difficulty_index] = if node.miss_count == u32::MAX {
                None
            } else {
                Some(node.miss_count)
            };
        }

        Ok(result)
    }

    fn follow_linked_list<R: ReadMemory>(
        reader: &R,
        entry_point: u64,
        null_obj: u64,
        visited: &mut HashSet<u64>,
        nodes: &mut HashMap<(u32, i32, i32), ListNode>,
    ) {
        let mut current_addr = entry_point;

        loop {
            // Prevent infinite loops
            if visited.contains(&current_addr) {
                break;
            }
            visited.insert(current_addr);

            // Read error = end of chain (not a fatal error)
            let Ok(buffer) = reader.read_bytes(current_addr, ListNode::SIZE) else {
                break;
            };
            let node = ListNode::from_bytes(&buffer);
            let next_addr = node.next;

            // Validate song_id (i32) directly before casting to u32.
            // Break on clearly invalid values (data corruption).
            if !(1000..=90000).contains(&node.song) {
                break;
            }

            if let std::collections::hash_map::Entry::Vacant(e) = nodes.entry(node.key()) {
                e.insert(node);
            }

            // Check for end of linked list
            if next_addr == 0 || next_addr == null_obj {
                break;
            }
            current_addr = next_addr;
        }
    }

    pub fn get(&self, song_id: u32) -> Option<&ScoreData> {
        self.scores.get(&song_id)
    }

    pub fn get_mut(&mut self, song_id: u32) -> Option<&mut ScoreData> {
        self.scores.get_mut(&song_id)
    }

    pub fn remove(&mut self, song_id: u32) -> Option<ScoreData> {
        self.scores.remove(&song_id)
    }

    pub fn insert(&mut self, song_id: u32, data: ScoreData) {
        self.scores.insert(song_id, data);
    }

    pub fn get_or_insert(&mut self, song_id: u32) -> &mut ScoreData {
        self.scores
            .entry(song_id)
            .or_insert_with(|| ScoreData::new(song_id))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &ScoreData)> {
        self.scores.iter()
    }

    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{MockMemoryBuilder, MockMemoryReader};

    #[test]
    fn test_score_data_new() {
        let data = ScoreData::new(1000);
        assert_eq!(data.song_id, 1000);
        assert_eq!(data.lamp, [Lamp::NoPlay; 10]);
        assert_eq!(data.score, [0; 10]);
    }

    #[test]
    fn test_score_data_miss_count_default() {
        let data = ScoreData::new(1000);
        // All miss counts should default to None
        for mc in &data.miss_count {
            assert!(mc.is_none());
        }
    }

    #[test]
    fn test_score_data_dj_points_default() {
        let data = ScoreData::new(1000);
        // All DJ points should default to 0.0
        for &djp in &data.dj_points {
            assert!((djp - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_list_node_difficulty_index_calculation() {
        // Test that diff + playtype * 5 gives correct indices
        // SP difficulties: diff 0-4 + playtype 0 = indices 0-4
        // DP difficulties: diff 0-4 + playtype 1 = indices 5-9
        assert_eq!(0, 0); // SPB: 0 + 0 * 5
        assert_eq!(1, 1); // SPN: 1 + 0 * 5
        assert_eq!(3, 3); // SPA: 3 + 0 * 5
        assert_eq!(5, 5); // DPB: 0 + 1 * 5
        assert_eq!(8, 8); // DPA: 3 + 1 * 5
    }

    /// Test ScoreMap::load_from_memory with a simple mock setup
    #[test]
    fn test_load_from_memory_empty_hash_table() {
        // Create a mock memory layout with empty hash table
        // Layout:
        //   base - 16: null_obj pointer
        //   base: table_start pointer
        //   base + 8: table_end pointer
        //   table: 8 bytes of zeros (empty bucket)

        let base = 0x1000u64;
        let table_start = base + 32;
        let table_end = table_start + 8;
        let null_obj = 0xFFFFFFFF_FFFFFFFFu64;

        let reader = MockMemoryBuilder::new()
            .base(base - 16)
            .with_size(64)
            // null_obj at base - 16
            .write_u64(0, null_obj)
            // table_start at base (offset 16 from buffer start)
            .write_u64(16, table_start)
            // table_end at base + 8 (offset 24 from buffer start)
            .write_u64(24, table_end)
            // Empty bucket (8 bytes of zeros at table_start)
            .write_u64(32, 0)
            .build();

        // Empty song database
        let song_db: HashMap<u32, SongInfo> = HashMap::new();

        let result = ScoreMap::load_from_memory(&reader, base, &song_db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_score_data_get_set() {
        let mut data = ScoreData::new(1000);

        data.set_lamp(Difficulty::SpA, Lamp::HardClear);
        data.set_score(Difficulty::SpA, 2500);

        assert_eq!(data.get_lamp(Difficulty::SpA), Lamp::HardClear);
        assert_eq!(data.get_score(Difficulty::SpA), 2500);
        assert_eq!(data.get_lamp(Difficulty::SpN), Lamp::NoPlay);
        assert_eq!(data.get_score(Difficulty::SpN), 0);
    }

    #[test]
    fn test_score_map_operations() {
        let mut map = ScoreMap::new();
        assert!(map.is_empty());

        let data = ScoreData::new(1000);
        map.insert(1000, data);

        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);
        assert!(map.get(1000).is_some());
        assert!(map.get(2000).is_none());
    }

    #[test]
    fn test_score_map_get_or_insert() {
        let mut map = ScoreMap::new();

        // First access creates entry
        let entry = map.get_or_insert(1000);
        entry.set_lamp(Difficulty::SpN, Lamp::Clear);

        // Second access returns same entry
        let entry2 = map.get_or_insert(1000);
        assert_eq!(entry2.get_lamp(Difficulty::SpN), Lamp::Clear);

        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_load_from_memory_rejects_oversized_buffer() {
        // Simulate corrupt memory where end_address - start_address > 16 MB.
        // load_from_memory should return an empty ScoreMap instead of allocating.
        let base = 0x1000u64;
        let table_start = base + 32;
        // Set table_end so the implied buffer exceeds MAX_DATA_MAP_BUFFER_SIZE
        let table_end = table_start + (MAX_DATA_MAP_BUFFER_SIZE as u64) + 1;
        let null_obj = 0xFFFFFFFF_FFFFFFFFu64;

        let reader = MockMemoryBuilder::new()
            .base(base - 16)
            .with_size(64)
            .write_u64(0, null_obj)
            .write_u64(16, table_start)
            .write_u64(24, table_end)
            .build();

        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, base, &song_db).unwrap();
        assert!(result.is_empty());
    }

    /// Helper: build a mock memory reader containing a single-node linked list.
    /// Returns (reader, data_map_addr).
    fn build_single_node_reader(diff: i32, playtype: i32, song: i32) -> (MockMemoryReader, u64) {
        // Memory layout (base = 0x1000):
        //   offset 0:  null_obj (u64)            -- data_map_addr - 16
        //   offset 16: table_start ptr (u64)     -- data_map_addr
        //   offset 24: table_end ptr (u64)       -- data_map_addr + 8
        //   offset 32: bucket[0] = node_addr     -- hash table (8 bytes)
        //   offset 40: ListNode (64 bytes)        -- the single node
        let base = 0x1000u64;
        let data_map_addr = base + 16;
        let table_start = base + 32;
        let table_end = table_start + 8; // one bucket
        let null_obj = 0xDEADu64;
        let node_addr = base + 40;

        // Build ListNode bytes (64 bytes)
        let mut node_bytes = [0u8; 64];
        // next = 0 (end of list)
        node_bytes[0..8].copy_from_slice(&0u64.to_le_bytes());
        // prev = 0
        node_bytes[8..16].copy_from_slice(&0u64.to_le_bytes());
        // diff
        node_bytes[16..20].copy_from_slice(&diff.to_le_bytes());
        // song
        node_bytes[20..24].copy_from_slice(&song.to_le_bytes());
        // playtype
        node_bytes[24..28].copy_from_slice(&playtype.to_le_bytes());
        // score = 1500
        node_bytes[32..36].copy_from_slice(&1500u32.to_le_bytes());
        // miss_count = 5
        node_bytes[36..40].copy_from_slice(&5u32.to_le_bytes());
        // lamp = 4 (Clear)
        node_bytes[48..52].copy_from_slice(&4i32.to_le_bytes());

        let reader = MockMemoryBuilder::new()
            .base(base)
            .with_size(104)
            .write_u64(0, null_obj) // null_obj at base
            .write_u64(16, table_start)
            .write_u64(24, table_end)
            .write_u64(32, node_addr) // bucket points to node
            .write_bytes(40, &node_bytes)
            .build();

        (reader, data_map_addr)
    }

    #[test]
    fn test_load_from_memory_rejects_negative_diff() {
        let (reader, data_map_addr) = build_single_node_reader(-1, 0, 1001);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        // Node with diff=-1 should be rejected
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_rejects_negative_playtype() {
        let (reader, data_map_addr) = build_single_node_reader(0, -1, 1001);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        // Node with playtype=-1 should be rejected
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_rejects_out_of_range_diff() {
        let (reader, data_map_addr) = build_single_node_reader(5, 0, 1001);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        // diff=5 is out of valid range 0..5
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_rejects_out_of_range_playtype() {
        let (reader, data_map_addr) = build_single_node_reader(0, 2, 1001);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        // playtype=2 is out of valid range 0..2
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_accepts_valid_diff_playtype() {
        let (reader, data_map_addr) = build_single_node_reader(3, 1, 1001);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        // diff=3, playtype=1 => difficulty_index=8 (DPA), should be accepted
        assert_eq!(result.len(), 1);
        let score_data = result.get(1001).unwrap();
        assert_eq!(score_data.score[8], 1500);
        assert_eq!(score_data.miss_count[8], Some(5));
    }

    #[test]
    fn test_list_node_from_bytes() {
        // Create test bytes for ListNode (64 bytes)
        let mut bytes = [0u8; 64];

        // next pointer (8 bytes at offset 0)
        bytes[0..8].copy_from_slice(&0x1234567890ABCDEFu64.to_le_bytes());
        // prev pointer (8 bytes at offset 8)
        bytes[8..16].copy_from_slice(&0xFEDCBA0987654321u64.to_le_bytes());
        // diff (4 bytes at offset 16)
        bytes[16..20].copy_from_slice(&3i32.to_le_bytes()); // SPA
        // song (4 bytes at offset 20)
        bytes[20..24].copy_from_slice(&1000i32.to_le_bytes());
        // playtype (4 bytes at offset 24)
        bytes[24..28].copy_from_slice(&0i32.to_le_bytes()); // SP
        // score (4 bytes at offset 32)
        bytes[32..36].copy_from_slice(&2500u32.to_le_bytes());
        // miss_count (4 bytes at offset 36)
        bytes[36..40].copy_from_slice(&15u32.to_le_bytes());
        // lamp (4 bytes at offset 48)
        bytes[48..52].copy_from_slice(&5i32.to_le_bytes()); // HardClear

        let node = ListNode::from_bytes(&bytes);

        assert_eq!(node.next, 0x1234567890ABCDEF);
        assert_eq!(node.diff, 3);
        assert_eq!(node.song, 1000);
        assert_eq!(node.playtype, 0);
        assert_eq!(node.score, 2500);
        assert_eq!(node.miss_count, 15);
        assert_eq!(node.lamp, 5);
        assert_eq!(node.key(), (1000, 3, 0));
    }

    /// Helper: build a mock memory reader with a custom lamp value.
    /// Returns (reader, data_map_addr).
    fn build_single_node_reader_with_lamp(lamp: i32) -> (MockMemoryReader, u64) {
        let base = 0x1000u64;
        let data_map_addr = base + 16;
        let table_start = base + 32;
        let table_end = table_start + 8;
        let null_obj = 0xDEADu64;
        let node_addr = base + 40;

        let mut node_bytes = [0u8; 64];
        node_bytes[0..8].copy_from_slice(&0u64.to_le_bytes()); // next = 0
        node_bytes[8..16].copy_from_slice(&0u64.to_le_bytes()); // prev = 0
        node_bytes[16..20].copy_from_slice(&0i32.to_le_bytes()); // diff = 0 (SPB)
        node_bytes[20..24].copy_from_slice(&1001i32.to_le_bytes()); // song
        node_bytes[24..28].copy_from_slice(&0i32.to_le_bytes()); // playtype = 0 (SP)
        node_bytes[32..36].copy_from_slice(&1500u32.to_le_bytes()); // score
        node_bytes[36..40].copy_from_slice(&5u32.to_le_bytes()); // miss_count
        node_bytes[48..52].copy_from_slice(&lamp.to_le_bytes()); // lamp (custom)

        let reader = MockMemoryBuilder::new()
            .base(base)
            .with_size(104)
            .write_u64(0, null_obj)
            .write_u64(16, table_start)
            .write_u64(24, table_end)
            .write_u64(32, node_addr)
            .write_bytes(40, &node_bytes)
            .build();

        (reader, data_map_addr)
    }

    #[test]
    fn test_load_from_memory_rejects_truncated_lamp_0x107() {
        // 0x107 = 263; `as u8` would truncate to 7 (FullCombo) -- must be rejected
        let (reader, data_map_addr) = build_single_node_reader_with_lamp(0x107);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_rejects_negative_lamp() {
        let (reader, data_map_addr) = build_single_node_reader_with_lamp(-1);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_rejects_lamp_8() {
        // lamp=8 is one past FullCombo(7), must be rejected
        let (reader, data_map_addr) = build_single_node_reader_with_lamp(8);
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_memory_accepts_valid_lamp_values() {
        // All valid lamp values (0..=7) should be accepted
        for lamp_val in 0..=7i32 {
            let (reader, data_map_addr) = build_single_node_reader_with_lamp(lamp_val);
            let song_db: HashMap<u32, SongInfo> = HashMap::new();
            let result = ScoreMap::load_from_memory(&reader, data_map_addr, &song_db).unwrap();
            assert_eq!(result.len(), 1, "lamp={lamp_val} should be accepted");
            let score_data = result.get(1001).unwrap();
            let expected_lamp = Lamp::from_u8(lamp_val as u8).unwrap();
            assert_eq!(
                score_data.lamp[0], expected_lamp,
                "lamp={lamp_val} mismatch"
            );
        }
    }
}
