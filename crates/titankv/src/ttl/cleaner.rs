use crate::concurrency::local_lock::StripedLocalLock;
use crate::concurrency::process_lock::ProcessLockManager;
use crate::concurrency::shard::ShardRouter;
use crate::error::TitanError;
use crate::index::bucket::BucketManager;
use crate::log::record::{encode_log_record, RECORD_TYPE_TOMBSTONE};
use crate::log::writer::LaneWriter;
use crate::ttl::event::TtlEvent;
use crate::ttl::heap::BoundedTtlHeap;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TtlCleaner {
    heap: Mutex<BoundedTtlHeap>,
    cleaned_count: AtomicU64,
}

impl TtlCleaner {
    pub fn new(heap_capacity: usize) -> Self {
        Self {
            heap: Mutex::new(BoundedTtlHeap::new(heap_capacity)),
            cleaned_count: AtomicU64::new(0),
        }
    }

    pub fn schedule_event(&self, event: TtlEvent) -> bool {
        self.heap.lock().push(event)
    }

    pub fn clean_expired(
        &self,
        router: &ShardRouter,
        bucket_mgr: &BucketManager,
        local_lock: &StripedLocalLock,
        process_lock: &ProcessLockManager,
        writers: &[Arc<LaneWriter>],
        sequence_generator: &AtomicU64,
        batch_limit: usize,
    ) -> Result<usize, TitanError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut cleaned = 0;
        let mut mut_heap = self.heap.lock();

        while cleaned < batch_limit {
            let next_item = if let Some(item) = mut_heap.peek_expired(now_ms) {
                Some(item.clone())
            } else {
                None
            };

            let item = match next_item {
                Some(it) => it,
                None => break,
            };

            mut_heap.pop();

            let route = router.route(&item.key);
            let _local_guard = local_lock.write(route.global_bucket_id);
            let _proc_guard = process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

            if let Ok(Some(rec)) = bucket_mgr.get_record(route.shard_id, route.local_bucket_id, &item.key, route.hash) {
                if rec.sequence == item.sequence && rec.expire_at == item.expire_at && rec.expire_at <= now_ms {
                    let seq = sequence_generator.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut log_buf = Vec::new();
                    encode_log_record(
                        RECORD_TYPE_TOMBSTONE,
                        0,
                        seq,
                        0,
                        rec.relocation_generation(),
                        &item.key,
                        0,
                        &[],
                        &mut log_buf,
                    );

                    let lane_idx = route.lane_id % writers.len();
                    let (seg_id, offset) = writers[lane_idx].append(&log_buf).unwrap_or((String::new(), 0));

                    let tombstone_rec = crate::index::record::IndexRecord {
                        hash: route.hash,
                        key: item.key.clone(),
                        state_segment_id: seg_id,
                        state_offset: offset,
                        state_relocation_generation: rec.relocation_generation(),
                        value_segment_id: String::new(),
                        value_offset: 0,
                        value_relocation_generation: 0,
                        stored_len: 0,
                        raw_len: 0,
                        expire_at: 0,
                        sequence: seq,
                        flags: crate::index::record::FLAG_TOMBSTONE,
                    };
                    let _ = bucket_mgr.upsert_record(route.shard_id, route.local_bucket_id, tombstone_rec);
                    cleaned += 1;
                }
            }
        }

        self.cleaned_count.fetch_add(cleaned as u64, Ordering::Relaxed);
        Ok(cleaned)
    }

    pub fn total_cleaned(&self) -> u64 {
        self.cleaned_count.load(Ordering::Relaxed)
    }
}
