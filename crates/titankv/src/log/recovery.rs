use crate::concurrency::shard::ShardRouter;
use crate::error::TitanError;
use crate::index::bucket::BucketManager;
use crate::index::record::{IndexRecord, FLAG_TOMBSTONE};
use crate::log::record::{
    decode_log_record, decode_ttl_update_payload, RECORD_TYPE_PUT, RECORD_TYPE_TOMBSTONE,
    RECORD_TYPE_TTL_UPDATE,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct RecoveryStats {
    pub segments_scanned: usize,
    pub records_replayed: usize,
    pub live_keys: usize,
    pub tombstones: usize,
    pub max_sequence: u64,
}

pub fn replay_logs<P: AsRef<Path>>(
    data_dir: P,
    router: &ShardRouter,
    bucket_mgr: &BucketManager,
) -> Result<RecoveryStats, TitanError> {
    let logs_dir = data_dir.as_ref().join("logs");
    if !logs_dir.exists() {
        return Ok(RecoveryStats {
            segments_scanned: 0,
            records_replayed: 0,
            live_keys: 0,
            tombstones: 0,
            max_sequence: 0,
        });
    }

    let mut segments_scanned = 0;
    let mut records_replayed = 0;
    let mut max_sequence = 0u64;

    // Track highest (sequence, relocation_generation) per key
    // Key -> (sequence, relocation_generation, IndexRecord, is_tombstone)
    let mut latest_records: HashMap<Vec<u8>, (u64, u32, IndexRecord, bool)> = HashMap::new();

    let process_dirs = match std::fs::read_dir(&logs_dir) {
        Ok(d) => d,
        Err(_) => {
            return Ok(RecoveryStats {
                segments_scanned: 0,
                records_replayed: 0,
                live_keys: 0,
                tombstones: 0,
                max_sequence: 0,
            })
        }
    };

    for proc_entry in process_dirs {
        let proc_entry = proc_entry?;
        if !proc_entry.file_type()?.is_dir() {
            continue;
        }

        let lane_dirs = std::fs::read_dir(proc_entry.path())?;
        for lane_entry in lane_dirs {
            let lane_entry = lane_entry?;
            if !lane_entry.file_type()?.is_dir() {
                continue;
            }

            let seg_files = std::fs::read_dir(lane_entry.path())?;
            for seg_file in seg_files {
                let seg_file = seg_file?;
                let path = seg_file.path();
                if path.extension().and_then(|s| s.to_str()) != Some("log") {
                    continue;
                }

                let seg_id = path.file_stem().unwrap().to_string_lossy().to_string();
                segments_scanned += 1;

                let mut file = File::open(&path)?;
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;

                let mut offset = 0;
                while offset < data.len() {
                    let remaining = &data[offset..];
                    match decode_log_record(remaining) {
                        Ok((header, key, val_slice, rec_len)) => {
                            records_replayed += 1;
                            if header.sequence > max_sequence {
                                max_sequence = header.sequence;
                            }
                            let hash = router.hash_key(key);

                            let (rec, is_tombstone) = match header.record_type {
                                RECORD_TYPE_PUT => {
                                    let rec = IndexRecord {
                                        hash,
                                        key: key.to_vec(),
                                        state_segment_id: seg_id.clone(),
                                        state_offset: offset as u64,
                                        value_segment_id: seg_id.clone(),
                                        value_offset: offset as u64,
                                        stored_len: header.stored_value_len,
                                        raw_len: header.raw_value_len,
                                        expire_at: header.expire_at,
                                        sequence: header.sequence,
                                        state_relocation_generation: header.relocation_generation,
                                        value_relocation_generation: header.relocation_generation,
                                        flags: header.flags,
                                    };
                                    (rec, false)
                                }
                                RECORD_TYPE_TOMBSTONE => {
                                    let rec = IndexRecord {
                                        hash,
                                        key: key.to_vec(),
                                        state_segment_id: seg_id.clone(),
                                        state_offset: offset as u64,
                                        value_segment_id: String::new(),
                                        value_offset: 0,
                                        stored_len: 0,
                                        raw_len: 0,
                                        expire_at: header.expire_at,
                                        sequence: header.sequence,
                                        state_relocation_generation: header.relocation_generation,
                                        value_relocation_generation: 0,
                                        flags: FLAG_TOMBSTONE,
                                    };
                                    (rec, true)
                                }
                                RECORD_TYPE_TTL_UPDATE => {
                                    if let Ok(ttl_meta) = decode_ttl_update_payload(val_slice, header.raw_value_len) {
                                        let rec = IndexRecord {
                                            hash,
                                            key: key.to_vec(),
                                            state_segment_id: seg_id.clone(),
                                            state_offset: offset as u64,
                                            value_segment_id: ttl_meta.value_segment_id,
                                            value_offset: ttl_meta.value_offset,
                                            stored_len: ttl_meta.stored_value_len,
                                            raw_len: ttl_meta.raw_value_len,
                                            expire_at: header.expire_at,
                                            sequence: header.sequence,
                                            state_relocation_generation: header.relocation_generation,
                                            value_relocation_generation: ttl_meta.value_relocation_generation,
                                            flags: ttl_meta.value_flags,
                                        };
                                        (rec, false)
                                    } else {
                                        offset += rec_len;
                                        continue;
                                    }
                                }
                                _ => {
                                    offset += rec_len;
                                    continue;
                                }
                            };

                            let key_vec = key.to_vec();
                            if let Some((cur_seq, cur_reloc, _, _)) = latest_records.get(&key_vec) {
                                if (header.sequence, header.relocation_generation) > (*cur_seq, *cur_reloc) {
                                    latest_records.insert(
                                        key_vec,
                                        (header.sequence, header.relocation_generation, rec, is_tombstone),
                                    );
                                }
                            } else {
                                latest_records.insert(
                                    key_vec,
                                    (header.sequence, header.relocation_generation, rec, is_tombstone),
                                );
                            }

                            offset += rec_len;
                        }
                        Err(_) => {
                            // Incomplete tail or corrupted record: stop processing this segment tail
                            break;
                        }
                    }
                }
            }
        }
    }

    // Apply latest records to index
    let mut live_keys = 0;
    let mut tombstones = 0;

    for (key, (_, _, rec, is_tombstone)) in latest_records {
        let route = router.route(&key);
        if is_tombstone {
            tombstones += 1;
            let _ = bucket_mgr.upsert_record(route.shard_id, route.local_bucket_id, rec);
        } else {
            live_keys += 1;
            let _ = bucket_mgr.upsert_record(route.shard_id, route.local_bucket_id, rec);
        }
    }

    Ok(RecoveryStats {
        segments_scanned,
        records_replayed,
        live_keys,
        tombstones,
        max_sequence,
    })
}
