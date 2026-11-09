use crate::concurrency::local_lock::StripedLocalLock;
use crate::concurrency::process_lock::ProcessLockManager;
use crate::concurrency::shard::ShardRouter;
use crate::error::TitanError;
use crate::index::bucket::BucketManager;
use crate::index::record::IndexRecord;
use crate::log::record::{decode_log_record, encode_log_record, RECORD_TYPE_PUT};
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
                    Ok((header, key, value, rec_len)) => {
                        if header.record_type == RECORD_TYPE_PUT {
                            let route = router.route(key);
                            let _local_guard = local_lock.write(route.global_bucket_id);
                            let _proc_guard = process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

                            if let Ok(Some(current_index)) = bucket_mgr.get_record(
                                route.shard_id,
                                route.local_bucket_id,
                                key,
                                route.hash,
                            ) {
                                // Verify it still points to this exact segment and offset
                                if current_index.segment_id == seg_id
                                    && current_index.value_offset == offset as u64
                                    && !current_index.is_expired(now_ms)
                                {
                                    // Relocate: preserve sequence, increment relocation_generation
                                    let new_reloc = current_index.relocation_generation + 1;
                                    let mut new_log_buf = Vec::new();
                                    encode_log_record(
                                        RECORD_TYPE_PUT,
                                        header.flags,
                                        header.sequence,
                                        header.expire_at,
                                        new_reloc,
                                        key,
                                        header.raw_value_len,
                                        value,
                                        &mut new_log_buf,
                                    );

                                    let lane_idx = route.lane_id % writers.len();
                                    if let Ok((new_seg_id, new_offset)) =
                                        writers[lane_idx].append(&new_log_buf)
                                    {
                                        let updated_index = IndexRecord {
                                            hash: current_index.hash,
                                            key: key.to_vec(),
                                            segment_id: new_seg_id,
                                            value_offset: new_offset,
                                            stored_len: header.stored_value_len,
                                            raw_len: header.raw_value_len,
                                            expire_at: header.expire_at,
                                            sequence: header.sequence,
                                            relocation_generation: new_reloc,
                                            flags: header.flags,
                                        };
                                        let _ = bucket_mgr.upsert_record(
                                            route.shard_id,
                                            route.local_bucket_id,
                                            updated_index,
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
