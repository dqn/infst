//! Utility functions for offset searching

/// Convert i32 values to little-endian byte representation
pub fn merge_byte_representations(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Check if a number is a power of two (used to filter out memory artifacts)
pub fn is_power_of_two(n: u32) -> bool {
    n > 0 && (n & (n - 1)) == 0
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

    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(256));
        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(5));
    }
}
