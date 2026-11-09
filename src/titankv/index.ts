/**
 * TitanKV - High-Performance Persistent KV Storage Engine
 * Node.js Native Bindings via napi-rs
 */
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);

let nativeAddon: any = null;
try {
  nativeAddon = require(path.resolve(process.cwd(), 'titankv.node'));
} catch (e1) {
  try {
    nativeAddon = require(path.resolve(process.cwd(), 'titankv.linux-x64-gnu.node'));
  } catch (e2) {
    try {
      nativeAddon = require(path.resolve(process.cwd(), 'crates/titankv/target/release/libtitankv.so'));
    } catch (e3) {
      console.warn('Native TitanKV addon loading notice:', (e3 as Error).message);
    }
  }
}

export interface TitanConfig {
  dataDir: string;

  // Global storage configuration
  global_shardCount?: number;
  global_bucketCount?: number;
  global_hashSeed?: number;
  global_segmentSizeBytes?: number;
  global_ttlBucketWidthMs?: number;
  global_maxKeyBytes?: number;
  global_maxValueBytes?: number;

  // Concurrency
  concurrency?: {
    writerLanesPerShard?: number;
    workerThreads?: number;
    maxConcurrentReads?: number;
    maxBatchSize?: number;
  };

  // Memory budget
  memory?: {
    maxBytes?: number;
    queueBytes?: number;
    inFlightBytes?: number;
    bufferPoolBytes?: number;
    pageBufferCount?: number;
  };

  // Write & durability
  write?: {
    flushIntervalMs?: number;
    flushBatchSize?: number;
    sync?: boolean;
  };

  // Compression
  compression?: {
    enabled?: boolean;
    algorithm?: 'lz4';
    minBytes?: number;
    minRatio?: number;
  };

  // TTL
  ttl?: {
    enabled?: boolean;
    heapCapacity?: number;
    cleanBatchSize?: number;
    cleanIntervalMs?: number;
    prefetchHorizonMs?: number;
    loadBatchSize?: number;
  };

  // GC
  gc?: {
    enabled?: boolean;
    maxLiveRatio?: number;
    batchSize?: number;
    intervalMs?: number;
    minSegmentAgeMs?: number;
  };

  // Recovery
  recovery?: {
    enabled?: boolean;
    mode?: 'full';
    verifyChecksum?: boolean;
  };

  // Default operations
  putMode?: 'overwrite' | 'skip' | 'ttl_compare';
  putTtlDiff?: number;
  getAllowExpired?: boolean;
  getwithTTL?: boolean;
  getwithExpireAt?: boolean;
}

export interface PutOptions {
  mode?: 'overwrite' | 'skip' | 'ttl_compare';
  ttl?: number;
  ttlDiff?: number;
  sync?: boolean;
}

export interface PutItem {
  key: string;
  value: Buffer | Uint8Array;
  options?: PutOptions;
}

export interface GetOptions {
  allowExpired?: boolean;
  withTTL?: boolean;
  withExpireAt?: boolean;
}

export interface GetResult {
  value: Buffer;
  ttl?: number;
  expireAt?: number;
}

export interface TtlOptions {
  ttl?: number;
  increase?: number;
  reduce?: number;
}

export interface TtlItem {
  key: string;
  ttl?: number;
  increase?: number;
  reduce?: number;
}

export interface TtlResult {
  ttl: number;
  expireAt: number;
}

export interface BatchResult {
  key: string;
  success: boolean;
  error?: string;
}

export interface EngineStats {
  total_keys: number;
  totalKeys: number;
  segments_count: number;
  segmentsCount: number;
  live_bytes: number;
  liveBytes: number;
  memory_used_bytes: number;
  memoryUsedBytes: number;
  ttl_cleaned_count: number;
  ttlCleanedCount: number;
  gc_cleaned_segments: number;
  gcCleanedSegments: number;
  gc_reclaimed_bytes: number;
  gcReclaimedBytes: number;
}

export class TitanDatabase {
  private native: any;
  private config: TitanConfig;

  constructor(nativeInstance: any, config: TitanConfig) {
    this.native = nativeInstance;
    this.config = config;
  }

  async put(key: string, buffer: Buffer | Uint8Array, options?: PutOptions): Promise<boolean> {
    const rawBuffer = Buffer.isBuffer(buffer) ? buffer : Buffer.from(buffer);
    const ttlDiff = options?.ttlDiff ?? this.config.putTtlDiff ?? 1800000;
    const opts = {
      mode: options?.mode ?? this.config.putMode ?? 'overwrite',
      ttl: options?.ttl,
      ttlDiff: ttlDiff,
      ttl_diff: ttlDiff,
      sync: options?.sync ?? this.config.write?.sync ?? false,
    };
    return await this.native.put(key, rawBuffer, opts);
  }

  async putMany(items: PutItem[], options?: PutOptions): Promise<BatchResult[]> {
    const defaultTtlDiff = options?.ttlDiff ?? this.config.putTtlDiff ?? 1800000;
    const nativeItems = items.map((it) => {
      const diff = it.options?.ttlDiff ?? defaultTtlDiff;
      return {
        key: it.key,
        value: Buffer.isBuffer(it.value) ? it.value : Buffer.from(it.value),
        options: it.options
          ? {
              mode: it.options.mode ?? this.config.putMode ?? 'overwrite',
              ttl: it.options.ttl,
              ttlDiff: diff,
              ttl_diff: diff,
              sync: it.options.sync ?? this.config.write?.sync ?? false,
            }
          : undefined,
      };
    });

    const defaultOpts = options
      ? {
          mode: options.mode ?? this.config.putMode ?? 'overwrite',
          ttl: options.ttl,
          ttlDiff: defaultTtlDiff,
          ttl_diff: defaultTtlDiff,
          sync: options.sync ?? this.config.write?.sync ?? false,
        }
      : undefined;

    return await this.native.putMany(nativeItems, defaultOpts);
  }

  async get(key: string, options?: GetOptions): Promise<Buffer | GetResult | null> {
    const withTTL = options?.withTTL ?? this.config.getwithTTL ?? false;
    const withExpireAt = options?.withExpireAt ?? this.config.getwithExpireAt ?? false;
    const allowExpired = options?.allowExpired ?? this.config.getAllowExpired ?? true;

    const res = await this.native.get(key, {
      allowExpired: allowExpired,
      allow_expired: allowExpired,
      withTtl: withTTL,
      with_ttl: withTTL,
      withExpireAt: withExpireAt,
      with_expire_at: withExpireAt,
    });

    if (!res) return null;

    if (withTTL || withExpireAt) {
      const out: GetResult = { value: res.value };
      if (withTTL) out.ttl = res.ttl;
      if (withExpireAt) out.expireAt = res.expireAt ?? res.expire_at;
      return out;
    }

    return res.value;
  }

  async getMany(keys: string[], options?: GetOptions): Promise<(Buffer | GetResult | null)[]> {
    const withTTL = options?.withTTL ?? this.config.getwithTTL ?? false;
    const withExpireAt = options?.withExpireAt ?? this.config.getwithExpireAt ?? false;
    const allowExpired = options?.allowExpired ?? this.config.getAllowExpired ?? true;

    const rawList = await this.native.getMany(keys, {
      allowExpired: allowExpired,
      allow_expired: allowExpired,
      withTtl: withTTL,
      with_ttl: withTTL,
      withExpireAt: withExpireAt,
      with_expire_at: withExpireAt,
    });

    return rawList.map((res: any) => {
      if (!res) return null;
      if (withTTL || withExpireAt) {
        const out: GetResult = { value: res.value };
        if (withTTL) out.ttl = res.ttl;
        if (withExpireAt) out.expireAt = res.expireAt ?? res.expire_at;
        return out;
      }
      return res.value;
    });
  }

  async delete(key: string): Promise<boolean> {
    return await this.native.delete(key);
  }

  async deleteMany(keys: string[]): Promise<boolean[]> {
    return await this.native.deleteMany(keys);
  }

  async ttl(key: string, options?: TtlOptions): Promise<TtlResult | null> {
    const res = await this.native.ttl(key, options);
    if (!res) return null;
    return {
      ttl: res.ttl,
      expireAt: res.expireAt ?? res.expire_at,
    };
  }

  async ttlMany(items: TtlItem[]): Promise<(TtlResult | null)[]> {
    const rawList = await this.native.ttlMany(items);
    return rawList.map((res: any) =>
      res ? { ttl: res.ttl, expireAt: res.expireAt ?? res.expire_at } : null
    );
  }

  async stats(): Promise<EngineStats> {
    const s = await this.native.stats();
    return {
      total_keys: s.totalKeys ?? s.total_keys ?? 0,
      totalKeys: s.totalKeys ?? s.total_keys ?? 0,
      segments_count: s.segmentsCount ?? s.segments_count ?? 0,
      segmentsCount: s.segmentsCount ?? s.segments_count ?? 0,
      live_bytes: s.liveBytes ?? s.live_bytes ?? 0,
      liveBytes: s.liveBytes ?? s.live_bytes ?? 0,
      memory_used_bytes: s.memoryUsedBytes ?? s.memory_used_bytes ?? 0,
      memoryUsedBytes: s.memoryUsedBytes ?? s.memory_used_bytes ?? 0,
      ttl_cleaned_count: s.ttlCleanedCount ?? s.ttl_cleaned_count ?? 0,
      ttlCleanedCount: s.ttlCleanedCount ?? s.ttl_cleaned_count ?? 0,
      gc_cleaned_segments: s.gcCleanedSegments ?? s.gc_cleaned_segments ?? 0,
      gcCleanedSegments: s.gcCleanedSegments ?? s.gc_cleaned_segments ?? 0,
      gc_reclaimed_bytes: s.gcReclaimedBytes ?? s.gc_reclaimed_bytes ?? 0,
      gcReclaimedBytes: s.gcReclaimedBytes ?? s.gc_reclaimed_bytes ?? 0,
    };
  }

  async close(): Promise<void> {
    await this.native.close();
  }
}

export async function open(config: TitanConfig): Promise<TitanDatabase> {
  if (!nativeAddon) {
    throw new Error('TitanKV native addon (.node) is not compiled or failed to load.');
  }

  // Format config for Rust serde_json
  const rustConfig = {
    data_dir: config.dataDir,
    global_shard_count: config.global_shardCount ?? 16,
    global_bucket_count: config.global_bucketCount ?? 4194304,
    global_hash_seed: config.global_hashSeed ?? 0,
    global_segment_size_bytes: config.global_segmentSizeBytes ?? 268435456,
    global_ttl_bucket_width_ms: config.global_ttlBucketWidthMs ?? 60000,
    global_max_key_bytes: config.global_maxKeyBytes ?? 64,
    global_max_value_bytes: config.global_maxValueBytes ?? 16777216,
    concurrency: {
      writer_lanes_per_shard: config.concurrency?.writerLanesPerShard ?? 4,
      worker_threads: config.concurrency?.workerThreads ?? 4,
      max_concurrent_reads: config.concurrency?.maxConcurrentReads ?? 64,
      max_batch_size: config.concurrency?.maxBatchSize ?? 10000,
    },
    memory: {
      max_bytes: config.memory?.maxBytes ?? 536870912,
      queue_bytes: config.memory?.queueBytes ?? 67108864,
      in_flight_bytes: config.memory?.inFlightBytes ?? 67108864,
      buffer_pool_bytes: config.memory?.bufferPoolBytes ?? 134217728,
      page_buffer_count: config.memory?.pageBufferCount ?? 128,
    },
    write: {
      flush_interval_ms: config.write?.flushIntervalMs ?? 1000,
      flush_batch_size: config.write?.flushBatchSize ?? 1000,
      sync: config.write?.sync ?? false,
    },
    compression: {
      enabled: config.compression?.enabled ?? true,
      min_bytes: config.compression?.minBytes ?? 2048,
      min_ratio: config.compression?.minRatio ?? 0.9,
    },
    ttl: {
      enabled: config.ttl?.enabled ?? true,
      heap_capacity: config.ttl?.heapCapacity ?? 100000,
      clean_batch_size: config.ttl?.cleanBatchSize ?? 1000,
      clean_interval_ms: config.ttl?.cleanIntervalMs ?? 1000,
      prefetch_horizon_ms: config.ttl?.prefetchHorizonMs ?? 300000,
      load_batch_size: config.ttl?.loadBatchSize ?? 10000,
    },
    gc: {
      enabled: config.gc?.enabled ?? true,
      max_live_ratio: config.gc?.maxLiveRatio ?? 0.5,
      batch_size: config.gc?.batchSize ?? 1000,
      interval_ms: config.gc?.intervalMs ?? 60000,
      min_segment_age_ms: config.gc?.minSegmentAgeMs ?? 300000,
    },
    recovery: {
      enabled: config.recovery?.enabled ?? true,
      mode: config.recovery?.mode ?? 'full',
      verify_checksum: config.recovery?.verifyChecksum ?? true,
    },
    put_mode: config.putMode ?? 'overwrite',
    put_ttl_diff: config.putTtlDiff ?? 1800000,
    get_allow_expired: config.getAllowExpired ?? true,
    get_with_ttl: config.getwithTTL ?? false,
    get_with_expire_at: config.getwithExpireAt ?? false,
  };

  const nativeEngine = nativeAddon.openNativeEngine(JSON.stringify(rustConfig));
  return new TitanDatabase(nativeEngine, config);
}

export default { open };
