use crate::error::TitanError;
use serde::{Deserialize, Serialize};

pub const FLAG_COMPRESSED: u16 = 0x0002;
pub const FLAG_LZ4: u16 = 0x0004;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub min_bytes: usize,
    pub min_ratio: f64,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_bytes: 2048,
            min_ratio: 0.9,
        }
    }
}

pub struct CompressedResult {
    pub data: Vec<u8>,
    pub flags: u16,
    pub raw_len: u32,
    pub stored_len: u32,
}

pub fn compress_value(
    value: &[u8],
    config: &CompressionConfig,
) -> CompressedResult {
    let raw_len = value.len() as u32;
    if !config.enabled || value.len() < config.min_bytes {
        return CompressedResult {
            data: value.to_vec(),
            flags: 0,
            raw_len,
            stored_len: raw_len,
        };
    }

    let compressed = lz4_flex::compress_prepend_size(value);
    let ratio = compressed.len() as f64 / value.len() as f64;

    if ratio < config.min_ratio {
        let stored_len = compressed.len() as u32;
        CompressedResult {
            data: compressed,
            flags: FLAG_COMPRESSED | FLAG_LZ4,
            raw_len,
            stored_len,
        }
    } else {
        CompressedResult {
            data: value.to_vec(),
            flags: 0,
            raw_len,
            stored_len: raw_len,
        }
    }
}

pub fn decompress_value(
    stored_data: &[u8],
    flags: u16,
    raw_len: u32,
) -> Result<Vec<u8>, TitanError> {
    if (flags & FLAG_COMPRESSED) != 0 {
        lz4_flex::decompress_size_prepended(stored_data)
            .map_err(|e| TitanError::CorruptedRecord(format!("LZ4 decompression failed: {}", e)))
    } else {
        if stored_data.len() != raw_len as usize {
            return Err(TitanError::CorruptedRecord(format!(
                "Raw length mismatch: expected {}, got {}",
                raw_len,
                stored_data.len()
            )));
        }
        Ok(stored_data.to_vec())
    }
}
