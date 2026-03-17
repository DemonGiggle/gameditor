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

/// Full scan: walk all readable regions and collect matching addresses.
pub fn full_scan(handle_raw: usize, target: &[u8]) -> Vec<Candidate> {
    let width = target.len() as u8;
    let regions = query_readable_regions(handle_raw);
    let mut candidates = Vec::new();

    for region in &regions {
        let Some(data) = read_bytes(handle_raw, region.base, region.size) else {
            continue;
        };
        if data.len() < target.len() {
            continue;
        }
        for i in 0..=(data.len() - target.len()) {
            if &data[i..i + target.len()] == target {
                candidates.push(Candidate {
                    address: region.base + i as u64,
                    width,
                    value: target.to_vec(),
                    pinned: false,
                });
            }
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
