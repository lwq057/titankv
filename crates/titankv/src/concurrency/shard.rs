use xxhash_rust::xxh64::xxh64;

#[derive(Debug, Clone, Copy)]
pub struct ShardRouter {
    shard_count: usize,
    bucket_count: usize,
    buckets_per_shard: usize,
    shard_bits: u32,
    writer_lanes_per_shard: usize,
    hash_seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteInfo {
    pub hash: u64,
    pub shard_id: usize,
    pub local_bucket_id: usize,
    pub global_bucket_id: usize,
    pub lane_id: usize,
}

impl ShardRouter {
    pub fn new(
        shard_count: usize,
        bucket_count: usize,
        writer_lanes_per_shard: usize,
        hash_seed: u64,
    ) -> Self {
        assert!(shard_count.is_power_of_two());
        assert!(bucket_count.is_power_of_two());
        assert!(bucket_count % shard_count == 0);
        let buckets_per_shard = bucket_count / shard_count;
        let shard_bits = shard_count.trailing_zeros();
        Self {
            shard_count,
            bucket_count,
            buckets_per_shard,
            shard_bits,
            writer_lanes_per_shard,
            hash_seed,
        }
    }

    #[inline]
    pub fn hash_key(&self, key: &[u8]) -> u64 {
        xxh64(key, self.hash_seed)
    }

    #[inline]
    pub fn route(&self, key: &[u8]) -> RouteInfo {
        let hash = self.hash_key(key);
        self.route_hash(hash)
    }

    #[inline]
    pub fn route_hash(&self, hash: u64) -> RouteInfo {
        let shard_id = (hash as usize) & (self.shard_count - 1);
        let local_bucket_id = ((hash >> self.shard_bits) as usize) & (self.buckets_per_shard - 1);
        let global_bucket_id = shard_id * self.buckets_per_shard + local_bucket_id;
        let lane_id = local_bucket_id % self.writer_lanes_per_shard;

        RouteInfo {
            hash,
            shard_id,
            local_bucket_id,
            global_bucket_id,
            lane_id,
        }
    }

    pub fn shard_count(&self) -> usize {
        self.shard_count
    }

    pub fn bucket_count(&self) -> usize {
        self.bucket_count
    }

    pub fn buckets_per_shard(&self) -> usize {
        self.buckets_per_shard
    }
}
