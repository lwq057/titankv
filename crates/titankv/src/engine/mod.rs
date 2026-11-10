pub mod lifecycle;

use crate::compression::{compress_value, decompress_value};
use crate::concurrency::lane::LaneManager;
use crate::concurrency::local_lock::StripedLocalLock;
use crate::concurrency::process_lock::ProcessLockManager;
use crate::concurrency::shard::ShardRouter;
use crate::concurrency::worker_pool::WorkerPool;
use crate::config::TitanConfig;
use crate::error::TitanError;
use crate::gc::collector::SegmentGcCollector;
use crate::index::allocator::OverflowAllocator;
use crate::index::bucket::BucketManager;
use crate::index::record::{IndexRecord, FLAG_TOMBSTONE};
use crate::log::record::{
    decode_log_record, encode_log_record, encode_ttl_update_record, LOG_HEADER_SIZE,
    LOG_TRAILER_SIZE, RECORD_TYPE_PUT, RECORD_TYPE_TOMBSTONE,
};
use crate::log::recovery::replay_logs;
use crate::log::segment::Segment;
use crate::log::writer::LaneWriter;
use crate::memory::budget::MemoryBudget;
use crate::memory::pool::BufferPool;
use crate::persistence::manifest::ManifestManager;
use crate::ttl::bucket::TtlDiskManager;
use crate::ttl::cleaner::TtlCleaner;
use crate::ttl::event::TtlEvent;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutOptions {
    pub mode: Option<String>,
    pub ttl: Option<i64>,
    pub ttl_diff: Option<i64>,
    pub sync: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutItem {
    pub key: String,
    pub value: Vec<u8>,
    pub options: Option<PutOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOptions {
    pub allow_expired: Option<bool>,
    pub with_ttl: Option<bool>,
    pub with_expire_at: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetResult {
    pub value: Vec<u8>,
    pub ttl: i64,
    pub expire_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlOptions {
    pub ttl: Option<i64>,
    pub increase: Option<i64>,
    pub reduce: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlItem {
    pub key: String,
    pub ttl: Option<i64>,
    pub increase: Option<i64>,
    pub reduce: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlResult {
    pub ttl: i64,
    pub expire_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub key: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    pub total_keys: usize,
    pub segments_count: usize,
    pub live_bytes: u64,
    pub memory_used_bytes: usize,
    pub ttl_cleaned_count: u64,
    pub gc_cleaned_segments: u64,
    pub gc_reclaimed_bytes: u64,
}

pub struct TitanEngine {
    pub config: TitanConfig,
    pub process_uuid: String,
    router: ShardRouter,
    local_lock: Arc<StripedLocalLock>,
    process_lock: Arc<ProcessLockManager>,
    lifecycle: Mutex<lifecycle::DatabaseLifecycle>,
    bucket_mgr: Arc<BucketManager>,
    lane_mgr: Arc<LaneManager>,
    worker_pool: Arc<WorkerPool>,
    writers: Vec<Arc<LaneWriter>>,
    open_segments: Arc<Mutex<HashMap<String, Arc<Segment>>>>,
    manifest_mgr: Arc<Mutex<ManifestManager>>,
    ttl_disk_mgrs: Vec<Arc<TtlDiskManager>>,
    ttl_cleaner: Arc<TtlCleaner>,
    gc_collector: Arc<SegmentGcCollector>,
    memory_budget: Arc<MemoryBudget>,
    #[allow(dead_code)]
    buffer_pool: Arc<BufferPool>,
    sequence_gen: Arc<AtomicU64>,
    total_keys: Arc<AtomicU64>,
    live_bytes: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

impl TitanEngine {
    pub fn open(config: TitanConfig) -> Result<Arc<Self>, TitanError> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir)?;

        let process_uuid = Uuid::new_v4().to_string();

        let router = ShardRouter::new(
            config.global.global_shard_count,
            config.global.global_bucket_count,
            config.concurrency.writer_lanes_per_shard,
            config.global.global_hash_seed,
        );

        let local_lock = Arc::new(StripedLocalLock::new());
        let process_lock = Arc::new(ProcessLockManager::new(&config.data_dir)?);
        let lifecycle = lifecycle::DatabaseLifecycle::initialize(Arc::clone(&process_lock))?;

        let manifest_mgr = Arc::new(Mutex::new(ManifestManager::load_or_create(
            &config.data_dir,
            &config.global,
        )?));

        // Create overflow allocators and bucket manager
        let mut allocators = Vec::with_capacity(config.global.global_shard_count);
        for s in 0..config.global.global_shard_count {
            let alloc = Arc::new(OverflowAllocator::new(
                &config.data_dir,
                s,
                router.buckets_per_shard(),
            )?);
            allocators.push(alloc);
        }
        let bucket_mgr = Arc::new(BucketManager::new(allocators, router.buckets_per_shard()));

        // Perform recovery replay if primary/sole process
        let mut initial_keys = 0u64;
        let mut initial_seq = 0u64;
        if lifecycle.is_primary_recovered {
            if let Ok(stats) = replay_logs(&config.data_dir, &router, &bucket_mgr) {
                initial_keys = stats.live_keys as u64;
                initial_seq = stats.max_sequence;
            }
        }

        let total_lanes = config.global.global_shard_count * config.concurrency.writer_lanes_per_shard;
        let lane_mgr = Arc::new(LaneManager::new(total_lanes));
        let worker_pool = Arc::new(WorkerPool::new(config.concurrency.worker_threads));

        let open_segments = Arc::new(Mutex::new(HashMap::new()));
        let mut writers = Vec::with_capacity(total_lanes);
        let mut ttl_disk_mgrs = Vec::with_capacity(total_lanes);

        for l in 0..total_lanes {
            let writer = Arc::new(LaneWriter::new(
                &config.data_dir,
                &process_uuid,
                l,
                config.global.global_segment_size_bytes,
                Arc::clone(&open_segments),
            )?);
            writers.push(writer);

            let ttl_mgr = Arc::new(TtlDiskManager::new(
                &config.data_dir,
                &process_uuid,
                l,
                config.global.global_ttl_bucket_width_ms,
            )?);
            ttl_disk_mgrs.push(ttl_mgr);
        }

        let memory_budget = Arc::new(MemoryBudget::new(&config.memory));
        let buffer_pool = Arc::new(BufferPool::new(
            Arc::clone(&memory_budget),
            config.memory.page_buffer_count,
        ));

        let ttl_cleaner = Arc::new(TtlCleaner::new(config.ttl.heap_capacity));
        let gc_collector = Arc::new(SegmentGcCollector::new());
        let sequence_gen = Arc::new(AtomicU64::new(initial_seq.max(100) + 10));
        let total_keys = Arc::new(AtomicU64::new(initial_keys));
        let live_bytes = Arc::new(AtomicU64::new(0));
        let closed = Arc::new(AtomicBool::new(false));

        let engine = Arc::new(Self {
            config,
            process_uuid,
            router,
            local_lock,
            process_lock,
            lifecycle: Mutex::new(lifecycle),
            bucket_mgr,
            lane_mgr,
            worker_pool,
            writers,
            open_segments,
            manifest_mgr,
            ttl_disk_mgrs,
            ttl_cleaner,
            gc_collector,
            memory_budget,
            buffer_pool,
            sequence_gen,
            total_keys,
            live_bytes,
            closed,
        });

        // Spawn periodic background cleaner and GC tasks
        if engine.config.ttl.enabled {
            let cleaner_engine = Arc::clone(&engine);
            let cleaner_interval = engine.config.ttl.clean_interval_ms;
            let cleaner_batch = engine.config.ttl.clean_batch_size;
            engine.worker_pool.spawn(move || {
                while !cleaner_engine.closed.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(cleaner_interval));
                    if cleaner_engine.closed.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = cleaner_engine.ttl_cleaner.clean_expired(
                        &cleaner_engine.router,
                        &cleaner_engine.bucket_mgr,
                        &cleaner_engine.local_lock,
                        &cleaner_engine.process_lock,
                        &cleaner_engine.writers,
                        &cleaner_engine.sequence_gen,
                        cleaner_batch,
                    );
                }
            });
        }

        Ok(engine)
    }

    pub fn put(
        &self,
        key: &str,
        value: &[u8],
        options: Option<&PutOptions>,
    ) -> Result<bool, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }

        let key_bytes = key.as_bytes();
        if key_bytes.is_empty() || key_bytes.len() > self.config.global.global_max_key_bytes {
            return Err(TitanError::InvalidKey(format!(
                "Key length must be 1..{}",
                self.config.global.global_max_key_bytes
            )));
        }

        if value.len() > self.config.global.global_max_value_bytes {
            return Err(TitanError::InvalidValue("Value exceeds maximum limit".into()));
        }

        // Memory budget check for in-flight allocation
        let _inflight_guard = self.memory_budget.acquire_inflight(value.len() + 256)?;

        let route = self.router.route(key_bytes);
        let mode = options
            .and_then(|o| o.mode.as_deref())
            .unwrap_or(&self.config.put_mode);
        let ttl_diff = options
            .and_then(|o| o.ttl_diff)
            .unwrap_or(self.config.put_ttl_diff);
        let sync_write = options
            .and_then(|o| o.sync)
            .unwrap_or(self.config.write.sync);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let global_lane_id = route.shard_id * self.config.concurrency.writer_lanes_per_shard + route.lane_id;
        let _lane_guard = self.lane_mgr.lock_lane(global_lane_id);

        // Strict lock ordering: Local Bucket Lock -> Process Bucket Lock
        let _local_guard = self.local_lock.write(route.global_bucket_id);
        let _proc_guard = self.process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

        // Read current index record
        let existing = self
            .bucket_mgr
            .get_record(route.shard_id, route.local_bucket_id, key_bytes, route.hash)?;

        // Calculate expireAt per Section 12.2:
        // - 新 Key 未指定 TTL：永久有效 (-1)
        // - 已有 Key 覆盖时未指定 TTL：保留原 TTL
        // - 显式 ttl=-1：永久有效
        // - 显式 ttl=0：立即过期
        let expire_at = match options.and_then(|o| o.ttl) {
            Some(-1) => -1,
            Some(0) => now_ms,
            Some(t) if t > 0 => now_ms + t,
            _ => {
                if let Some(ref old_rec) = existing {
                    if !old_rec.is_tombstone() && !old_rec.is_expired(now_ms) {
                        old_rec.expire_at
                    } else {
                        -1
                    }
                } else {
                    -1
                }
            }
        };

        if let Some(ref old_rec) = existing {
            let old_valid = !old_rec.is_tombstone() && !old_rec.is_expired(now_ms);

            match mode {
                "skip" => {
                    if old_valid {
                        return Ok(false);
                    }
                }
                "ttl_compare" => {
                    if options.and_then(|o| o.ttl).is_none() {
                        return Err(TitanError::InvalidConfig("ttl_compare mode requires ttl option".into()));
                    }
                    if old_valid {
                        if expire_at == -1 {
                            // New key is permanent: allowed to overwrite finite TTL
                        } else if old_rec.expire_at == -1 {
                            // Old is permanent, new is finite: reject
                            return Ok(false);
                        } else {
                            // Both are finite
                            if expire_at - old_rec.expire_at < ttl_diff {
                                return Ok(false);
                            }
                        }
                    }
                }
                _ => {} // "overwrite"
            }
        } else if mode == "ttl_compare" && options.and_then(|o| o.ttl).is_none() {
            return Err(TitanError::InvalidConfig("ttl_compare mode requires ttl option".into()));
        }

        // Compress value if applicable
        let compressed = compress_value(value, &self.config.compression);

        // Sequence number allocation inside bucket lock - strictly monotonic per key
        let old_seq = existing.as_ref().map(|r| r.sequence).unwrap_or(0);
        let allocated = self.sequence_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let seq = if allocated <= old_seq {
            let next_seq = old_seq + 1;
            self.sequence_gen.fetch_max(next_seq + 1, Ordering::SeqCst);
            next_seq
        } else {
            allocated
        };

        // Encode and append Log Record
        let mut log_buf = Vec::new();
        encode_log_record(
            RECORD_TYPE_PUT,
            compressed.flags,
            seq,
            expire_at,
            0,
            key_bytes,
            compressed.raw_len,
            &compressed.data,
            &mut log_buf,
        );

        let lane_idx = global_lane_id % self.writers.len();
        let writer = &self.writers[lane_idx];
        let (seg_id, offset) = writer.append(&log_buf)?;

        if sync_write {
            writer.sync_current()?;
        }

        // Append TTL Event if key has finite TTL
        if expire_at > 0 {
            let ttl_event = TtlEvent {
                expire_at,
                shard_id: route.shard_id as u16,
                local_bucket_id: route.local_bucket_id as u32,
                sequence: seq,
                key: key_bytes.to_vec(),
            };
            let _ = self.ttl_disk_mgrs[lane_idx].append_event(&ttl_event);
            self.ttl_cleaner.schedule_event(ttl_event);
        }

        // Update Index Record with dual pointers
        let is_new = existing.as_ref().map(|r| r.is_tombstone()).unwrap_or(true);
        let index_rec = IndexRecord {
            hash: route.hash,
            key: key_bytes.to_vec(),
            state_segment_id: seg_id.clone(),
            state_offset: offset,
            value_segment_id: seg_id,
            value_offset: offset,
            stored_len: compressed.stored_len,
            raw_len: compressed.raw_len,
            expire_at,
            sequence: seq,
            state_relocation_generation: 0,
            value_relocation_generation: 0,
            flags: compressed.flags,
        };

        self.bucket_mgr
            .upsert_record(route.shard_id, route.local_bucket_id, index_rec)?;

        if is_new {
            self.total_keys.fetch_add(1, Ordering::Relaxed);
        }
        self.live_bytes.fetch_add(compressed.raw_len as u64, Ordering::Relaxed);

        Ok(true)
    }

    pub fn put_many(
        &self,
        items: &[PutItem],
        options: Option<&PutOptions>,
    ) -> Result<Vec<BatchResult>, TitanError> {
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let opts = item.options.as_ref().or(options);
            match self.put(&item.key, &item.value, opts) {
                Ok(success) => results.push(BatchResult {
                    key: item.key.clone(),
                    success,
                    error: None,
                }),
                Err(err) => results.push(BatchResult {
                    key: item.key.clone(),
                    success: false,
                    error: Some(err.to_string()),
                }),
            }
        }
        Ok(results)
    }

    pub fn get(
        &self,
        key: &str,
        options: Option<&GetOptions>,
    ) -> Result<Option<GetResult>, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }

        let key_bytes = key.as_bytes();
        if key_bytes.is_empty() || key_bytes.len() > self.config.global.global_max_key_bytes {
            return Err(TitanError::InvalidKey("Invalid key length".into()));
        }

        let allow_expired = options
            .and_then(|o| o.allow_expired)
            .unwrap_or(self.config.get_allow_expired);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let route = self.router.route(key_bytes);

        // Reader lock ordering: Local Shared -> Process Shared
        let _local_guard = self.local_lock.read(route.global_bucket_id);
        let _proc_guard = self.process_lock.lock_bucket_shared(route.global_bucket_id)?;

        let index_rec = match self
            .bucket_mgr
            .get_record(route.shard_id, route.local_bucket_id, key_bytes, route.hash)?
        {
            Some(rec) => rec,
            None => return Ok(None),
        };

        if index_rec.is_tombstone() {
            return Ok(None);
        }

        if !allow_expired && index_rec.is_expired(now_ms) {
            return Ok(None);
        }

        // Get segment arc and pin it
        let seg_arc = {
            let mut segs = self.open_segments.lock();
            if let Some(s) = segs.get(&index_rec.value_segment_id) {
                Arc::clone(s)
            } else {
                let seg = Segment::open_by_id(
                    &self.config.data_dir,
                    &index_rec.value_segment_id,
                )?;
                segs.insert(index_rec.value_segment_id.clone(), Arc::clone(&seg));
                seg
            }
        };

        let _pin = seg_arc.pin();

        // Release Bucket Locks early before reading and decompressing value!
        drop(_proc_guard);
        drop(_local_guard);

        // Read log record from segment using header + key + stored value + trailer
        let total_rec_len = LOG_HEADER_SIZE + (index_rec.key.len()) + (index_rec.stored_len as usize) + LOG_TRAILER_SIZE;
        let mut rec_buf = vec![0u8; total_rec_len];
        seg_arc.read_exact_at(&mut rec_buf, index_rec.value_offset)?;

        let (header, _decoded_key, stored_val, _) = decode_log_record(&rec_buf)?;

        let val_bytes = decompress_value(stored_val, header.flags, header.raw_value_len)?;

        let ttl_remaining = if index_rec.expire_at == -1 {
            -1
        } else if index_rec.expire_at > 0 {
            (index_rec.expire_at - now_ms).max(0)
        } else {
            -1
        };

        Ok(Some(GetResult {
            value: val_bytes,
            ttl: ttl_remaining,
            expire_at: index_rec.expire_at,
        }))
    }

    pub fn get_many(
        &self,
        keys: &[String],
        options: Option<&GetOptions>,
    ) -> Result<Vec<Option<GetResult>>, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }
        let mut results = Vec::with_capacity(keys.len());
        for k in keys {
            match self.get(k, options) {
                Ok(res) => results.push(res),
                Err(_) => results.push(None),
            }
        }
        Ok(results)
    }

    pub fn delete(&self, key: &str) -> Result<bool, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }

        let key_bytes = key.as_bytes();
        if key_bytes.is_empty() || key_bytes.len() > self.config.global.global_max_key_bytes {
            return Err(TitanError::InvalidKey("Invalid key length".into()));
        }

        let route = self.router.route(key_bytes);
        let global_lane_id = route.shard_id * self.config.concurrency.writer_lanes_per_shard + route.lane_id;
        let _lane_guard = self.lane_mgr.lock_lane(global_lane_id);

        let _local_guard = self.local_lock.write(route.global_bucket_id);
        let _proc_guard = self.process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

        let existing = self
            .bucket_mgr
            .get_record(route.shard_id, route.local_bucket_id, key_bytes, route.hash)?;

        let old_seq = existing.as_ref().map(|r| r.sequence).unwrap_or(0);
        let allocated = self.sequence_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let seq = if allocated <= old_seq {
            let next_seq = old_seq + 1;
            self.sequence_gen.fetch_max(next_seq + 1, Ordering::SeqCst);
            next_seq
        } else {
            allocated
        };
        let mut log_buf = Vec::new();
        encode_log_record(
            RECORD_TYPE_TOMBSTONE,
            FLAG_TOMBSTONE,
            seq,
            0,
            0,
            key_bytes,
            0,
            &[],
            &mut log_buf,
        );

        let lane_idx = global_lane_id % self.writers.len();
        let writer = &self.writers[lane_idx];
        let (tomb_seg_id, tomb_offset) = writer.append(&log_buf)?;

        let was_active = if let Some(ref old_rec) = existing {
            !old_rec.is_tombstone()
        } else {
            false
        };

        let tombstone_rec = IndexRecord {
            hash: route.hash,
            key: key_bytes.to_vec(),
            state_segment_id: tomb_seg_id,
            state_offset: tomb_offset,
            value_segment_id: String::new(),
            value_offset: 0,
            stored_len: 0,
            raw_len: 0,
            expire_at: 0,
            sequence: seq,
            state_relocation_generation: 0,
            value_relocation_generation: 0,
            flags: FLAG_TOMBSTONE,
        };

        self.bucket_mgr
            .upsert_record(route.shard_id, route.local_bucket_id, tombstone_rec)?;

        if was_active {
            let mut cur = self.total_keys.load(Ordering::Relaxed);
            while cur > 0 {
                match self.total_keys.compare_exchange_weak(
                    cur,
                    cur - 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => cur = actual,
                }
            }
        }

        Ok(true)
    }

    pub fn delete_many(&self, keys: &[String]) -> Result<Vec<bool>, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }
        let mut results = Vec::with_capacity(keys.len());
        for k in keys {
            match self.delete(k) {
                Ok(res) => results.push(res),
                Err(_) => results.push(false),
            }
        }
        Ok(results)
    }

    pub fn ttl(&self, key: &str, options: Option<&TtlOptions>) -> Result<Option<TtlResult>, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }

        let key_bytes = key.as_bytes();
        let route = self.router.route(key_bytes);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if options.is_none() || (options.unwrap().ttl.is_none() && options.unwrap().increase.is_none() && options.unwrap().reduce.is_none()) {
            // Read-only TTL query
            let _local_guard = self.local_lock.read(route.global_bucket_id);
            let _proc_guard = self.process_lock.lock_bucket_shared(route.global_bucket_id)?;

            if let Some(rec) = self.bucket_mgr.get_record(route.shard_id, route.local_bucket_id, key_bytes, route.hash)? {
                if rec.is_tombstone() {
                    return Ok(None);
                }
                let ttl_rem = if rec.expire_at == -1 {
                    -1
                } else if rec.expire_at > 0 {
                    (rec.expire_at - now_ms).max(0)
                } else {
                    -1
                };
                return Ok(Some(TtlResult {
                    ttl: ttl_rem,
                    expire_at: rec.expire_at,
                }));
            }
            return Ok(None);
        }

        // Update TTL
        let opts = options.unwrap();
        let global_lane_id = route.shard_id * self.config.concurrency.writer_lanes_per_shard + route.lane_id;
        let _lane_guard = self.lane_mgr.lock_lane(global_lane_id);

        let _local_guard = self.local_lock.write(route.global_bucket_id);
        let _proc_guard = self.process_lock.lock_bucket_exclusive(route.global_bucket_id)?;

        let mut rec = match self.bucket_mgr.get_record(route.shard_id, route.local_bucket_id, key_bytes, route.hash)? {
            Some(r) => r,
            None => return Ok(None),
        };

        if rec.is_tombstone() {
            return Ok(None);
        }

        let new_expire_at = if let Some(new_ttl) = opts.ttl {
            if new_ttl <= 0 {
                now_ms
            } else {
                now_ms + new_ttl
            }
        } else if let Some(inc) = opts.increase {
            if rec.expire_at == -1 {
                -1 // Permanent remains permanent
            } else {
                rec.expire_at + inc
            }
        } else if let Some(red) = opts.reduce {
            if rec.expire_at == -1 {
                return Err(TitanError::TtlNotSet);
            }
            (rec.expire_at - red).max(now_ms)
        } else {
            rec.expire_at
        };

        let allocated = self.sequence_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let seq = if allocated <= rec.sequence {
            let next_seq = rec.sequence + 1;
            self.sequence_gen.fetch_max(next_seq + 1, Ordering::SeqCst);
            next_seq
        } else {
            allocated
        };

        // Log TTL_UPDATE into Value Log (Section 4.2 & 8.5)
        let mut log_buf = Vec::new();
        encode_ttl_update_record(
            seq,
            new_expire_at,
            0,
            key_bytes,
            &rec.value_segment_id,
            rec.value_offset,
            rec.stored_len,
            rec.raw_len,
            rec.flags,
            rec.value_relocation_generation,
            &mut log_buf,
        );

        let lane_idx = global_lane_id % self.writers.len();
        let writer = &self.writers[lane_idx];
        let (ttl_seg_id, ttl_offset) = writer.append(&log_buf)?;

        rec.state_segment_id = ttl_seg_id;
        rec.state_offset = ttl_offset;
        rec.expire_at = new_expire_at;
        rec.sequence = seq;
        rec.state_relocation_generation = 0;

        // Log TTL update event
        if new_expire_at > 0 {
            let ttl_event = TtlEvent {
                expire_at: new_expire_at,
                shard_id: route.shard_id as u16,
                local_bucket_id: route.local_bucket_id as u32,
                sequence: rec.sequence,
                key: key_bytes.to_vec(),
            };
            let _ = self.ttl_disk_mgrs[lane_idx].append_event(&ttl_event);
            self.ttl_cleaner.schedule_event(ttl_event);
        }

        self.bucket_mgr.upsert_record(route.shard_id, route.local_bucket_id, rec)?;

        let ttl_rem = if new_expire_at == -1 {
            -1
        } else if new_expire_at > 0 {
            (new_expire_at - now_ms).max(0)
        } else {
            -1
        };

        Ok(Some(TtlResult {
            ttl: ttl_rem,
            expire_at: new_expire_at,
        }))
    }

    pub fn ttl_many(&self, items: &[TtlItem]) -> Result<Vec<Option<TtlResult>>, TitanError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(TitanError::DatabaseClosed);
        }
        let mut results = Vec::with_capacity(items.len());
        for it in items {
            let opts = TtlOptions {
                ttl: it.ttl,
                increase: it.increase,
                reduce: it.reduce,
            };
            match self.ttl(&it.key, Some(&opts)) {
                Ok(res) => results.push(res),
                Err(_) => results.push(None),
            }
        }
        Ok(results)
    }

    pub fn stats(&self) -> EngineStats {
        let (gc_cleaned_segs, gc_reclaimed) = self.gc_collector.stats();
        let segments_count = self.open_segments.lock().len();

        EngineStats {
            total_keys: self.total_keys.load(Ordering::Relaxed) as usize,
            segments_count,
            live_bytes: self.live_bytes.load(Ordering::Relaxed),
            memory_used_bytes: self.memory_budget.total_used(),
            ttl_cleaned_count: self.ttl_cleaner.total_cleaned(),
            gc_cleaned_segments: gc_cleaned_segs,
            gc_reclaimed_bytes: gc_reclaimed,
        }
    }

    pub fn close(&self) -> Result<(), TitanError> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        // Flush all lane writers
        for w in &self.writers {
            let _ = w.sync_current();
        }

        // Save manifest
        let _ = self.manifest_mgr.lock().save();

        // Release lifecycle lock
        self.lifecycle.lock().close();

        Ok(())
    }
}

impl Drop for TitanEngine {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
