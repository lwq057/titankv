use crate::compression::CompressionConfig;
use crate::error::TitanError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStorageConfig {
    #[serde(default = "default_shard_count")]
    pub global_shard_count: usize,

    #[serde(default = "default_bucket_count")]
    pub global_bucket_count: usize,

    #[serde(default = "default_hash_seed")]
    pub global_hash_seed: u64,

    #[serde(default = "default_segment_size")]
    pub global_segment_size_bytes: u64,

    #[serde(default = "default_ttl_bucket_width")]
    pub global_ttl_bucket_width_ms: u64,

    #[serde(default = "default_max_key_bytes")]
    pub global_max_key_bytes: usize,

    #[serde(default = "default_max_value_bytes")]
    pub global_max_value_bytes: usize,
}

fn default_shard_count() -> usize { 16 }
fn default_bucket_count() -> usize { 4194304 }
fn default_hash_seed() -> u64 { 0 }
fn default_segment_size() -> u64 { 268435456 } // 256MB
fn default_ttl_bucket_width() -> u64 { 60000 } // 60s
fn default_max_key_bytes() -> usize { 64 }
fn default_max_value_bytes() -> usize { 16777216 } // 16MB

impl Default for GlobalStorageConfig {
    fn default() -> Self {
        Self {
            global_shard_count: default_shard_count(),
            global_bucket_count: default_bucket_count(),
            global_hash_seed: default_hash_seed(),
            global_segment_size_bytes: default_segment_size(),
            global_ttl_bucket_width_ms: default_ttl_bucket_width(),
            global_max_key_bytes: default_max_key_bytes(),
            global_max_value_bytes: default_max_value_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    #[serde(default = "default_writer_lanes")]
    pub writer_lanes_per_shard: usize,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    #[serde(default = "default_max_concurrent_reads")]
    pub max_concurrent_reads: usize,
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,
}

fn default_writer_lanes() -> usize { 4 }
fn default_worker_threads() -> usize { 4 }
fn default_max_concurrent_reads() -> usize { 64 }
fn default_max_batch_size() -> usize { 10000 }

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            writer_lanes_per_shard: default_writer_lanes(),
            worker_threads: default_worker_threads(),
            max_concurrent_reads: default_max_concurrent_reads(),
            max_batch_size: default_max_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_max")]
    pub max_bytes: usize,
    #[serde(default = "default_memory_queue")]
    pub queue_bytes: usize,
    #[serde(default = "default_memory_inflight")]
    pub in_flight_bytes: usize,
    #[serde(default = "default_memory_buffer_pool")]
    pub buffer_pool_bytes: usize,
    #[serde(default = "default_page_buffer_count")]
    pub page_buffer_count: usize,
}

fn default_memory_max() -> usize { 536870912 } // 512MB
fn default_memory_queue() -> usize { 67108864 } // 64MB
fn default_memory_inflight() -> usize { 67108864 } // 64MB
fn default_memory_buffer_pool() -> usize { 134217728 } // 128MB
fn default_page_buffer_count() -> usize { 128 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_bytes: default_memory_max(),
            queue_bytes: default_memory_queue(),
            in_flight_bytes: default_memory_inflight(),
            buffer_pool_bytes: default_memory_buffer_pool(),
            page_buffer_count: default_page_buffer_count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteConfig {
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,
    #[serde(default = "default_flush_batch_size")]
    pub flush_batch_size: usize,
    #[serde(default = "default_sync")]
    pub sync: bool,
}

fn default_flush_interval() -> u64 { 1000 }
fn default_flush_batch_size() -> usize { 1000 }
fn default_sync() -> bool { false }

impl Default for WriteConfig {
    fn default() -> Self {
        Self {
            flush_interval_ms: default_flush_interval(),
            flush_batch_size: default_flush_batch_size(),
            sync: default_sync(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    #[serde(default = "default_ttl_enabled")]
    pub enabled: bool,
    #[serde(default = "default_heap_capacity")]
    pub heap_capacity: usize,
    #[serde(default = "default_clean_batch_size")]
    pub clean_batch_size: usize,
    #[serde(default = "default_clean_interval")]
    pub clean_interval_ms: u64,
    #[serde(default = "default_prefetch_horizon")]
    pub prefetch_horizon_ms: u64,
    #[serde(default = "default_load_batch_size")]
    pub load_batch_size: usize,
}

fn default_ttl_enabled() -> bool { true }
fn default_heap_capacity() -> usize { 100000 }
fn default_clean_batch_size() -> usize { 1000 }
fn default_clean_interval() -> u64 { 1000 }
fn default_prefetch_horizon() -> u64 { 300000 }
fn default_load_batch_size() -> usize { 10000 }

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            enabled: default_ttl_enabled(),
            heap_capacity: default_heap_capacity(),
            clean_batch_size: default_clean_batch_size(),
            clean_interval_ms: default_clean_interval(),
            prefetch_horizon_ms: default_prefetch_horizon(),
            load_batch_size: default_load_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    #[serde(default = "default_gc_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_live_ratio")]
    pub max_live_ratio: f64,
    #[serde(default = "default_gc_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_gc_interval")]
    pub interval_ms: u64,
    #[serde(default = "default_min_segment_age")]
    pub min_segment_age_ms: u64,
}

fn default_gc_enabled() -> bool { true }
fn default_max_live_ratio() -> f64 { 0.5 }
fn default_gc_batch_size() -> usize { 1000 }
fn default_gc_interval() -> u64 { 60000 }
fn default_min_segment_age() -> u64 { 300000 }

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            enabled: default_gc_enabled(),
            max_live_ratio: default_max_live_ratio(),
            batch_size: default_gc_batch_size(),
            interval_ms: default_gc_interval(),
            min_segment_age_ms: default_min_segment_age(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    #[serde(default = "default_recovery_enabled")]
    pub enabled: bool,
    #[serde(default = "default_recovery_mode")]
    pub mode: String,
    #[serde(default = "default_verify_checksum")]
    pub verify_checksum: bool,
}

fn default_recovery_enabled() -> bool { true }
fn default_recovery_mode() -> String { "full".to_string() }
fn default_verify_checksum() -> bool { true }

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: default_recovery_enabled(),
            mode: default_recovery_mode(),
            verify_checksum: default_verify_checksum(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitanConfig {
    pub data_dir: PathBuf,

    #[serde(flatten)]
    pub global: GlobalStorageConfig,

    #[serde(default)]
    pub concurrency: ConcurrencyConfig,

    #[serde(default)]
    pub memory: MemoryConfig,

    #[serde(default)]
    pub write: WriteConfig,

    #[serde(default)]
    pub compression: CompressionConfig,

    #[serde(default)]
    pub ttl: TtlConfig,

    #[serde(default)]
    pub gc: GcConfig,

    #[serde(default)]
    pub recovery: RecoveryConfig,

    // Default operations
    #[serde(default = "default_put_mode")]
    pub put_mode: String,
    #[serde(default = "default_put_ttl_diff")]
    pub put_ttl_diff: i64,
    #[serde(default = "default_get_allow_expired")]
    pub get_allow_expired: bool,
    #[serde(default = "default_get_with_ttl")]
    pub get_with_ttl: bool,
    #[serde(default = "default_get_with_expire_at")]
    pub get_with_expire_at: bool,
}

fn default_put_mode() -> String { "overwrite".to_string() }
fn default_put_ttl_diff() -> i64 { 1800000 }
fn default_get_allow_expired() -> bool { true }
fn default_get_with_ttl() -> bool { false }
fn default_get_with_expire_at() -> bool { false }

impl TitanConfig {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            global: GlobalStorageConfig::default(),
            concurrency: ConcurrencyConfig::default(),
            memory: MemoryConfig::default(),
            write: WriteConfig::default(),
            compression: CompressionConfig::default(),
            ttl: TtlConfig::default(),
            gc: GcConfig::default(),
            recovery: RecoveryConfig::default(),
            put_mode: default_put_mode(),
            put_ttl_diff: default_put_ttl_diff(),
            get_allow_expired: default_get_allow_expired(),
            get_with_ttl: default_get_with_ttl(),
            get_with_expire_at: default_get_with_expire_at(),
        }
    }

    pub fn validate(&self) -> Result<(), TitanError> {
        if self.data_dir.as_os_str().is_empty() {
            return Err(TitanError::InvalidConfig("dataDir cannot be empty".into()));
        }
        if !self.global.global_shard_count.is_power_of_two() {
            return Err(TitanError::InvalidConfig(
                "global_shardCount must be a power of two".into(),
            ));
        }
        if !self.global.global_bucket_count.is_power_of_two() {
            return Err(TitanError::InvalidConfig(
                "global_bucketCount must be a power of two".into(),
            ));
        }
        if self.global.global_bucket_count % self.global.global_shard_count != 0 {
            return Err(TitanError::InvalidConfig(
                "global_bucketCount must be divisible by global_shardCount".into(),
            ));
        }
        if self.global.global_max_key_bytes == 0 || self.global.global_max_key_bytes > 64 {
            return Err(TitanError::InvalidConfig(
                "global_maxKeyBytes must be between 1 and 64".into(),
            ));
        }
        if self.global.global_max_value_bytes == 0 {
            return Err(TitanError::InvalidConfig(
                "global_maxValueBytes must be > 0".into(),
            ));
        }
        if self.global.global_segment_size_bytes < (self.global.global_max_value_bytes as u64 + 4096) {
            return Err(TitanError::InvalidConfig(
                "global_segmentSizeBytes must be larger than maxValueBytes".into(),
            ));
        }
        if self.concurrency.writer_lanes_per_shard == 0 {
            return Err(TitanError::InvalidConfig("writerLanesPerShard must be > 0".into()));
        }
        if self.concurrency.worker_threads == 0 {
            return Err(TitanError::InvalidConfig("workerThreads must be > 0".into()));
        }
        if self.concurrency.max_batch_size == 0 {
            return Err(TitanError::InvalidConfig("maxBatchSize must be > 0".into()));
        }
        if self.ttl.heap_capacity == 0 {
            return Err(TitanError::InvalidConfig("ttl.heapCapacity must be > 0".into()));
        }
        if self.global.global_ttl_bucket_width_ms == 0 {
            return Err(TitanError::InvalidConfig("global_ttlBucketWidthMs must be > 0".into()));
        }
        if self.memory.queue_bytes > self.memory.max_bytes
            || self.memory.in_flight_bytes > self.memory.max_bytes
            || self.memory.buffer_pool_bytes > self.memory.max_bytes
        {
            return Err(TitanError::InvalidConfig(
                "Memory sub-budgets cannot exceed maxBytes".into(),
            ));
        }
        Ok(())
    }
}
