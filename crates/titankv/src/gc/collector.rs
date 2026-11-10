use crate::concurrency::local_lock::StripedLocalLock;
use crate::concurrency::process_lock::ProcessLockManager;
use crate::concurrency::shard::ShardRouter;
use crate::error::TitanError;
use crate::index::bucket::BucketManager;
use crate::index::record::FLAG_TOMBSTONE;
use crate::log::record::{
    decode_log_record, encode_log_record, encode_ttl_update_record,
    RECORD_TYPE_PUT, RECORD_TYPE_TOMBSTONE, RECORD_TYPE_TTL_UPDATE,
};
use crate::log::segment::Segment;
use crate::log::writer::LaneWriter;
use crate::persistence::manifest::ManifestManager;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SegmentGcCollector {
    cleaned_segments_count: AtomicU64,
    reclaimed_bytes: AtomicU64,
}

impl SegmentGcCollector {
    pub fn new() -> Self {
        Self {
            cleaned_segments_count: AtomicU64::new(0),
            reclaimed_bytes: AtomicU64::new(0),
        }
    }

    pub fn run_gc_pass(
        &self,
        router: &ShardRouter,
        bucket_mgr: &BucketManager,
        local_lock: &StripedLocalLock,
        process_lock: &ProcessLockManager,
        writers: &[Arc<LaneWriter>],
        open_segments: &Arc<Mutex<HashMap<String, Arc<Segment>>>>,
        manifest_mgr: &Arc<Mutex<ManifestManager>>,
        max_live_ratio: f64,
        batch_limit: usize,
    ) -> Result<usize, TitanError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Find candidate segments
        let candidate_segment_ids: Vec<String> = {
            let manifest = manifest_mgr.lock();
            manifest
                .data
                .segments
                .iter()
                .filter(|(_, meta)| {
                    meta.sealed
                        && meta.total_bytes > 0
                        && ((meta.live_bytes as f64) / (meta.total_bytes as f64)) < max_live_ratio
                })
                .take(1)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut relocated = 0;

        for seg_id in candidate_segment_ids {
            let seg_arc = {
                let segs = open_segments.lock();
                segs.get(&seg_id).cloned()
            };

            let seg = match seg_arc {
                Some(s) => s,
                None => continue,
            };

            // Read the entire segment
            let mut file = match File::open(&seg.file_path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let mut data = Vec::new();
            if file.read_to_end(&mut data).is_err() {
                continue;
            }

            let mut offset = 0;
            while offset < data.len() && relocated < batch_limit {
                let remaining = &data[offset..];
                match decode_log_record(remaining) {
                    Ok((header, key, value_slice, rec_len)) => {
                        let route = router.route(key);
                        let _local_guard = local_lock.write(route.global_bucket_id);
                        let _proc_guard = process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

                        if let Ok(Some(mut current_index)) = bucket_mgr.get_record(
                            route.shard_id,
                            route.local_bucket_id,
                            key,
                            route.hash,
                        ) {
                            let lane_idx = route.lane_id % writers.len();

                            if header.record_type == RECORD_TYPE_PUT {
                                // Check if this segment contains the active Value
                                if current_index.value_segment_id == seg_id
                                    && current_index.value_offset == offset as u64
                                    && !current_index.is_expired(now_ms)
                                    && !current_index.is_tombstone()
                                {
                                    let new_val_reloc = current_index.value_relocation_generation + 1;
                                    let mut new_log_buf = Vec::new();
                                    encode_log_record(
                                        RECORD_TYPE_PUT,
                                        header.flags,
                                        header.sequence,
                                        header.expire_at,
                                        new_val_reloc,
                                        key,
                                        header.raw_value_len,
                                        value_slice,
                                        &mut new_log_buf,
                                    );

                                    if let Ok((new_seg_id, new_offset)) = writers[lane_idx].append(&new_log_buf) {
                                        current_index.value_segment_id = new_seg_id.clone();
                                        current_index.value_offset = new_offset;
                                        current_index.value_relocation_generation = new_val_reloc;
                                        if current_index.state_segment_id == seg_id
                                            && current_index.state_offset == offset as u64
                                        {
                                            current_index.state_segment_id = new_seg_id;
                                            current_index.state_offset = new_offset;
                                            current_index.state_relocation_generation = new_val_reloc;
                                        }
                                        let _ = bucket_mgr.upsert_record(
                                            route.shard_id,
                                            route.local_bucket_id,
                                            current_index,
                                        );
                                        relocated += 1;
                                    }
                                }
                            } else if header.record_type == RECORD_TYPE_TTL_UPDATE {
                                // Check if this segment contains the active TTL_UPDATE state
                                if current_index.state_segment_id == seg_id
                                    && current_index.state_offset == offset as u64
                                    && !current_index.is_expired(now_ms)
                                    && !current_index.is_tombstone()
                                {
                                    let new_state_reloc = current_index.state_relocation_generation + 1;
                                    let mut new_log_buf = Vec::new();
                                    encode_ttl_update_record(
                                        header.sequence,
                                        header.expire_at,
                                        new_state_reloc,
                                        key,
                                        &current_index.value_segment_id,
                                        current_index.value_offset,
                                        current_index.stored_len,
                                        current_index.raw_len,
                                        current_index.flags,
                                        current_index.value_relocation_generation,
                                        &mut new_log_buf,
                                    );

                                    if let Ok((new_seg_id, new_offset)) = writers[lane_idx].append(&new_log_buf) {
                                        current_index.state_segment_id = new_seg_id;
                                        current_index.state_offset = new_offset;
                                        current_index.state_relocation_generation = new_state_reloc;
                                        let _ = bucket_mgr.upsert_record(
                                            route.shard_id,
                                            route.local_bucket_id,
                                            current_index,
                                        );
                                        relocated += 1;
                                    }
                                }
                            } else if header.record_type == RECORD_TYPE_TOMBSTONE {
                                if current_index.state_segment_id == seg_id
                                    && current_index.state_offset == offset as u64
                                    && current_index.is_tombstone()
                                {
                                    let new_state_reloc = current_index.state_relocation_generation + 1;
                                    let mut new_log_buf = Vec::new();
                                    encode_log_record(
                                        RECORD_TYPE_TOMBSTONE,
                                        FLAG_TOMBSTONE,
                                        header.sequence,
                                        0,
                                        new_state_reloc,
                                        key,
                                        0,
                                        &[],
                                        &mut new_log_buf,
                                    );

                                    if let Ok((new_seg_id, new_offset)) = writers[lane_idx].append(&new_log_buf) {
                                        current_index.state_segment_id = new_seg_id;
                                        current_index.state_offset = new_offset;
                                        current_index.state_relocation_generation = new_state_reloc;
                                        let _ = bucket_mgr.upsert_record(
                                            route.shard_id,
                                            route.local_bucket_id,
                                            current_index,
                                        );
                                        relocated += 1;
                                    }
                                }
                            }
                        }

                        offset += rec_len;
                    }
                    Err(_) => break,
                }
            }

            // If reader pins == 0 and all live records migrated, remove segment
            if seg.reader_count() == 0 {
                let file_len = seg.total_bytes.load(Ordering::Relaxed);
                let _ = std::fs::remove_file(&seg.file_path);
                open_segments.lock().remove(&seg_id);
                let _ = manifest_mgr.lock().remove_segment(&seg_id);
                self.cleaned_segments_count.fetch_add(1, Ordering::Relaxed);
                self.reclaimed_bytes.fetch_add(file_len, Ordering::Relaxed);
            }
        }

        Ok(relocated)
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.cleaned_segments_count.load(Ordering::Relaxed),
            self.reclaimed_bytes.load(Ordering::Relaxed),
        )
    }
}
