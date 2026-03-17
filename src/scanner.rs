use crate::memory::{query_readable_regions, read_bytes};
use crate::types::Candidate;

/// Encode `val` as little-endian bytes of the given width (1/2/4/8).
pub fn encode_value(val: u64, width: u8) -> Vec<u8> {
    match width {
        1 => (val as u8).to_le_bytes().to_vec(),
        2 => (val as u16).to_le_bytes().to_vec(),
        4 => (val as u32).to_le_bytes().to_vec(),
        8 => val.to_le_bytes().to_vec(),
        _ => vec![],
    }
}

/// Decode little-endian bytes into a u64 display value.
pub fn decode_value(bytes: &[u8]) -> u64 {
    let mut arr = [0u8; 8];
    let n = bytes.len().min(8);
    arr[..n].copy_from_slice(&bytes[..n]);
    u64::from_le_bytes(arr)
}

/// Search `data` for all occurrences of `target`, returning offsets relative to `base_addr`.
pub fn search_buffer(data: &[u8], target: &[u8], base_addr: u64) -> Vec<u64> {
    if target.is_empty() || data.len() < target.len() {
        return vec![];
    }
    (0..=(data.len() - target.len()))
        .filter(|&i| &data[i..i + target.len()] == target)
        .map(|i| base_addr + i as u64)
        .collect()
}

/// Full scan: walk all readable regions and collect matching addresses.
pub fn full_scan(handle_raw: usize, target: &[u8]) -> Vec<Candidate> {
    let width = target.len() as u8;
    let regions = query_readable_regions(handle_raw);
    let mut candidates = Vec::new();

    for region in &regions {
        let Some(data) = read_bytes(handle_raw, region.base, region.size) else {
            continue;
        };
        for addr in search_buffer(&data, target, region.base) {
            candidates.push(Candidate {
                address: addr,
                width,
                value: target.to_vec(),
                pinned: false,
            });
        }
    }
    candidates
}

/// Re-scan: re-read each existing candidate and keep only those matching `target`.
pub fn filter_scan(handle_raw: usize, candidates: &[Candidate], target: &[u8]) -> Vec<Candidate> {
    let width = target.len() as u8;
    candidates
        .iter()
        .filter_map(|c| {
            let data = read_bytes(handle_raw, c.address, target.len())?;
            if data == target {
                Some(Candidate {
                    address: c.address,
                    width,
                    value: data,
                    pinned: c.pinned,
                })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── encode_value ──────────────────────────────────────────────────────

    #[test]
    fn encode_u8() {
        assert_eq!(encode_value(0x42, 1), vec![0x42]);
    }

    #[test]
    fn encode_u16_le() {
        assert_eq!(encode_value(0x0102, 2), vec![0x02, 0x01]);
    }

    #[test]
    fn encode_u32_le() {
        assert_eq!(encode_value(0x01020304, 4), vec![0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn encode_u64_le() {
        assert_eq!(
            encode_value(0x0102030405060708, 8),
            vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
    }

    #[test]
    fn encode_zero() {
        assert_eq!(encode_value(0, 4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn encode_truncates_to_width() {
        // 256 as u8 wraps to 0
        assert_eq!(encode_value(256, 1), vec![0x00]);
        // 0x10001 as u16 wraps to 1
        assert_eq!(encode_value(0x10001, 2), vec![0x01, 0x00]);
    }

    #[test]
    fn encode_invalid_width_returns_empty() {
        assert_eq!(encode_value(42, 3), vec![]);
        assert_eq!(encode_value(42, 0), vec![]);
    }

    // ── decode_value ──────────────────────────────────────────────────────

    #[test]
    fn decode_u8() {
        assert_eq!(decode_value(&[0x42]), 0x42);
    }

    #[test]
    fn decode_u16() {
        assert_eq!(decode_value(&[0x02, 0x01]), 0x0102);
    }

    #[test]
    fn decode_u32() {
        assert_eq!(decode_value(&[0x04, 0x03, 0x02, 0x01]), 0x01020304);
    }

    #[test]
    fn decode_u64() {
        assert_eq!(
            decode_value(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]),
            0x0102030405060708
        );
    }

    #[test]
    fn decode_empty_is_zero() {
        assert_eq!(decode_value(&[]), 0);
    }

    #[test]
    fn decode_clamps_to_8_bytes() {
        // Extra bytes beyond 8 are ignored; value is same as first 8 bytes.
        let long = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF];
        assert_eq!(decode_value(&long), 1);
    }

    // ── encode / decode round-trips ───────────────────────────────────────

    #[test]
    fn roundtrip_u8() {
        for v in [0u64, 1, 127, 255] {
            assert_eq!(decode_value(&encode_value(v, 1)), v);
        }
    }

    #[test]
    fn roundtrip_u16() {
        for v in [0u64, 1, 1000, 65535] {
            assert_eq!(decode_value(&encode_value(v, 2)), v);
        }
    }

    #[test]
    fn roundtrip_u32() {
        for v in [0u64, 1, 100, u32::MAX as u64] {
            assert_eq!(decode_value(&encode_value(v, 4)), v);
        }
    }

    #[test]
    fn roundtrip_u64() {
        for v in [0u64, 1, u32::MAX as u64, u64::MAX] {
            assert_eq!(decode_value(&encode_value(v, 8)), v);
        }
    }

    // ── search_buffer ─────────────────────────────────────────────────────

    #[test]
    fn search_finds_match_at_start() {
        let data = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(search_buffer(&data, &[0x01, 0x02], 0x1000), vec![0x1000]);
    }

    #[test]
    fn search_finds_match_at_end() {
        let data = [0x00, 0x00, 0x01, 0x02];
        assert_eq!(search_buffer(&data, &[0x01, 0x02], 0x1000), vec![0x1002]);
    }

    #[test]
    fn search_finds_multiple_matches() {
        let data = [0xAB, 0x00, 0xAB, 0x00, 0xAB];
        let addrs = search_buffer(&data, &[0xAB], 0x0);
        assert_eq!(addrs, vec![0, 2, 4]);
    }

    #[test]
    fn search_no_match_returns_empty() {
        let data = [0x01, 0x02, 0x03];
        assert!(search_buffer(&data, &[0xFF], 0x0).is_empty());
    }

    #[test]
    fn search_target_longer_than_data() {
        let data = [0x01, 0x02];
        assert!(search_buffer(&data, &[0x01, 0x02, 0x03], 0x0).is_empty());
    }

    #[test]
    fn search_empty_target_returns_empty() {
        let data = [0x01, 0x02, 0x03];
        assert!(search_buffer(&data, &[], 0x0).is_empty());
    }

    #[test]
    fn search_exact_size_match() {
        let data = [0x01, 0x02, 0x03];
        assert_eq!(search_buffer(&data, &[0x01, 0x02, 0x03], 0x5000), vec![0x5000]);
    }

    #[test]
    fn search_exact_size_no_match() {
        let data = [0x01, 0x02, 0x04];
        assert!(search_buffer(&data, &[0x01, 0x02, 0x03], 0x0).is_empty());
    }

    #[test]
    fn search_base_addr_is_applied() {
        let data = [0x00, 0xFF, 0x00];
        assert_eq!(search_buffer(&data, &[0xFF], 0xDEAD0000), vec![0xDEAD0001]);
    }

    #[test]
    fn search_u32_value_in_buffer() {
        // Encode 1234 as u32 le, embed it in a buffer, check it's found.
        let target = encode_value(1234, 4);
        let mut data = vec![0xAAu8; 8];
        data[2..6].copy_from_slice(&target);
        let addrs = search_buffer(&data, &target, 0x2000);
        assert_eq!(addrs, vec![0x2002]);
    }
}
