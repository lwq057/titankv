use crate::checksum::{crc32c_calculate, crc32c_verify};
use crate::config::GlobalStorageConfig;
use crate::error::TitanError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MANIFEST_MAGIC: [u8; 4] = *b"TMNF";
pub const MANIFEST_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMeta {
    pub segment_id: String,
    pub lane_id: usize,
    pub process_uuid: String,
    pub created_at: u64,
    pub total_bytes: u64,
    pub live_bytes: u64,
    pub record_count: u64,
    pub sealed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestData {
    pub generation: u64,
    pub global_config: GlobalStorageConfig,
    pub segments: HashMap<String, SegmentMeta>,
}

pub struct ManifestManager {
    path: PathBuf,
    pub data: ManifestData,
}

impl ManifestManager {
    pub fn load_or_create<P: AsRef<Path>>(
        data_dir: P,
        global_config: &GlobalStorageConfig,
    ) -> Result<Self, TitanError> {
        let manifest_path = data_dir.as_ref().join("manifest");
        if manifest_path.exists() {
            let mut file = File::open(&manifest_path)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;

            if bytes.len() < 10 {
                return Err(TitanError::CorruptedRecord("Manifest file too short".into()));
            }

            if &bytes[0..4] != &MANIFEST_MAGIC {
                return Err(TitanError::CorruptedRecord("Invalid manifest magic".into()));
            }

            let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
            if version != MANIFEST_VERSION {
                return Err(TitanError::CorruptedRecord(format!(
                    "Unsupported manifest version: {}",
                    version
                )));
            }

            let payload_len = bytes.len() - 4;
            let checksum = u32::from_le_bytes(bytes[payload_len..].try_into().unwrap());
            if !crc32c_verify(&bytes[0..payload_len], checksum) {
                return Err(TitanError::ChecksumError);
            }

            let json_str = std::str::from_utf8(&bytes[6..payload_len])
                .map_err(|e| TitanError::CorruptedRecord(e.to_string()))?;
            let data: ManifestData = serde_json::from_str(json_str)
                .map_err(|e| TitanError::CorruptedRecord(e.to_string()))?;

            // Validate global config compatibility
            if data.global_config.global_shard_count != global_config.global_shard_count
                || data.global_config.global_bucket_count != global_config.global_bucket_count
                || data.global_config.global_hash_seed != global_config.global_hash_seed
                || data.global_config.global_max_key_bytes != global_config.global_max_key_bytes
                || data.global_config.global_max_value_bytes != global_config.global_max_value_bytes
            {
                return Err(TitanError::ConfigMismatch(
                    "Global storage config differs from existing manifest".into(),
                ));
            }

            Ok(Self {
                path: manifest_path,
                data,
            })
        } else {
            let data = ManifestData {
                generation: 1,
                global_config: global_config.clone(),
                segments: HashMap::new(),
            };
            let mut manager = Self {
                path: manifest_path,
                data,
            };
            manager.save()?;
            Ok(manager)
        }
    }

    pub fn save(&mut self) -> Result<(), TitanError> {
        self.data.generation += 1;
        let json_bytes = serde_json::to_vec(&self.data)
            .map_err(|e| TitanError::InternalError(e.to_string()))?;

        let mut buf = Vec::with_capacity(6 + json_bytes.len() + 4);
        buf.extend_from_slice(&MANIFEST_MAGIC);
        buf.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        buf.extend_from_slice(&json_bytes);

        let checksum = crc32c_calculate(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        let tmp_path = self.path.with_extension("tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            file.write_all(&buf)?;
            file.sync_all()?;
        }

        std::fs::rename(&tmp_path, &self.path)?;

        // Sync directory to ensure atomic durability
        if let Some(parent) = self.path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    }

    pub fn register_segment(&mut self, meta: SegmentMeta) -> Result<(), TitanError> {
        self.data.segments.insert(meta.segment_id.clone(), meta);
        self.save()
    }

    pub fn update_segment_live_bytes(&mut self, seg_id: &str, delta: i64) -> Result<(), TitanError> {
        if let Some(seg) = self.data.segments.get_mut(seg_id) {
            let new_live = (seg.live_bytes as i64 + delta).max(0) as u64;
            seg.live_bytes = new_live;
        }
        Ok(())
    }

    pub fn remove_segment(&mut self, seg_id: &str) -> Result<(), TitanError> {
        self.data.segments.remove(seg_id);
        self.save()
    }
}
