import path from 'node:path';
import fs from 'node:fs';
import { open, TitanDatabase, TitanConfig } from '../titankv/index.ts';

const DEFAULT_DATA_DIR = path.resolve(process.cwd(), '.titankv_data');

class TitanKvService {
  private db: TitanDatabase | null = null;
  private config: TitanConfig;
  private isBenchmarking = false;

  constructor() {
    this.config = {
      dataDir: DEFAULT_DATA_DIR,
      global_shardCount: 16,
      global_bucketCount: 1024,
      global_segmentSizeBytes: 64 * 1024 * 1024, // 64MB
      compression: {
        enabled: true,
        minBytes: 1024,
        minRatio: 0.9,
      },
      concurrency: {
        writerLanesPerShard: 4,
        workerThreads: 4,
        maxConcurrentReads: 64,
        maxBatchSize: 5000,
      },
      ttl: {
        enabled: true,
        heapCapacity: 50000,
        cleanBatchSize: 500,
        cleanIntervalMs: 1000,
      },
    };
  }

  async getDb(): Promise<TitanDatabase> {
    if (!this.db) {
      if (!fs.existsSync(this.config.dataDir)) {
        fs.mkdirSync(this.config.dataDir, { recursive: true });
      }
      this.db = await open(this.config);
    }
    return this.db;
  }

  async put(key: string, value: string | object, options?: any) {
    const db = await this.getDb();
    const strVal = typeof value === 'string' ? value : JSON.stringify(value);
    const buf = Buffer.from(strVal);
    const success = await db.put(key, buf, options);
    return { success, key, byteLength: buf.length };
  }

  async get(key: string, options?: any) {
    const db = await this.getDb();
    const res = await db.get(key, options);
    if (!res) return null;

    if (Buffer.isBuffer(res)) {
      const raw = res.toString('utf-8');
      try {
        return { value: JSON.parse(raw), isJson: true, rawLength: res.length };
      } catch {
        return { value: raw, isJson: false, rawLength: res.length };
      }
    } else {
      const valBuf = res.value;
      const raw = valBuf.toString('utf-8');
      let parsed: any = raw;
      let isJson = false;
      try {
        parsed = JSON.parse(raw);
        isJson = true;
      } catch {}

      return {
        value: parsed,
        isJson,
        rawLength: valBuf.length,
        ttl: res.ttl,
        expireAt: res.expireAt,
      };
    }
  }

  async delete(key: string) {
    const db = await this.getDb();
    const success = await db.delete(key);
    return { success, key };
  }

  async ttl(key: string, options?: any) {
    const db = await this.getDb();
    const res = await db.ttl(key, options);
    return res;
  }

  async stats() {
    const db = await this.getDb();
    const stats = await db.stats();

    // Inspect disk directory
    let totalDiskBytes = 0;
    let logFilesCount = 0;
    try {
      const walkDir = (dir: string) => {
        if (!fs.existsSync(dir)) return;
        const entries = fs.readdirSync(dir, { withFileTypes: true });
        for (const e of entries) {
          const fullPath = path.join(dir, e.name);
          if (e.isDirectory()) {
            walkDir(fullPath);
          } else {
            const st = fs.statSync(fullPath);
            totalDiskBytes += st.size;
            if (e.name.endsWith('.log')) logFilesCount++;
          }
        }
      };
      walkDir(this.config.dataDir);
    } catch {}

    return {
      ...stats,
      totalDiskBytes,
      logFilesCount,
      dataDir: this.config.dataDir,
      shards: this.config.global_shardCount,
      compressionEnabled: this.config.compression?.enabled ?? true,
      writerLanes: (this.config.global_shardCount || 16) * (this.config.concurrency?.writerLanesPerShard || 4),
    };
  }

  async runBenchmark(params: {
    totalKeys: number;
    batchSize: number;
    concurrency: number;
    payloadSize: number;
    ttlMs?: number;
  }) {
    if (this.isBenchmarking) {
      throw new Error('Another benchmark is currently running');
    }
    this.isBenchmarking = true;
    try {
      const db = await this.getDb();
      const totalKeys = Math.min(Math.max(params.totalKeys || 1000, 100), 50000);
      const batchSize = Math.min(Math.max(params.batchSize || 50, 1), 500);
      const payloadSize = Math.min(Math.max(params.payloadSize || 256, 32), 65536);
      const ttlMs = params.ttlMs ?? 300000;

      // Sample template data
      const sampleItem = {
        name: 'Benchmark JSON Payload',
        meta: 'TitanKV Storage Test',
        padding: 'X'.repeat(Math.max(0, payloadSize - 80)),
        ts: Date.now(),
      };
      const jsonBuffer = Buffer.from(JSON.stringify(sampleItem));

      // 1. Benchmark Writes
      const writeLatencies: number[] = [];
      const startWrite = performance.now();
      const totalBatches = Math.ceil(totalKeys / batchSize);

      for (let b = 0; b < totalBatches; b++) {
        const batchStart = performance.now();
        const items = [];
        const count = Math.min(batchSize, totalKeys - b * batchSize);
        for (let i = 0; i < count; i++) {
          const id = b * batchSize + i;
          items.push({
            key: `bench:live:${id}`,
            value: jsonBuffer,
            options: { ttl: ttlMs },
          });
        }
        await db.putMany(items);
        const batchElapsed = performance.now() - batchStart;
        for (let i = 0; i < count; i++) {
          writeLatencies.push(batchElapsed / count);
        }
      }
      const totalWriteTime = performance.now() - startWrite;

      // 2. Benchmark Reads
      const readLatencies: number[] = [];
      const startRead = performance.now();

      for (let b = 0; b < totalBatches; b++) {
        const batchStart = performance.now();
        const keys = [];
        const count = Math.min(batchSize, totalKeys - b * batchSize);
        for (let i = 0; i < count; i++) {
          const id = b * batchSize + i;
          keys.push(`bench:live:${id}`);
        }
        await db.getMany(keys);
        const batchElapsed = performance.now() - batchStart;
        for (let i = 0; i < count; i++) {
          readLatencies.push(batchElapsed / count);
        }
      }
      const totalReadTime = performance.now() - startRead;

      // Calculate Percentiles
      writeLatencies.sort((a, b) => a - b);
      readLatencies.sort((a, b) => a - b);

      const p50 = (arr: number[]) => arr[Math.floor(arr.length * 0.5)] || 0;
      const p95 = (arr: number[]) => arr[Math.floor(arr.length * 0.95)] || 0;
      const p99 = (arr: number[]) => arr[Math.floor(arr.length * 0.99)] || 0;
      const max = (arr: number[]) => arr[arr.length - 1] || 0;

      const writeQps = Math.round((totalKeys / (totalWriteTime / 1000)));
      const readQps = Math.round((totalKeys / (totalReadTime / 1000)));

      const stats = await this.stats();

      return {
        totalKeys,
        batchSize,
        payloadSize,
        write: {
          qps: writeQps,
          durationMs: Math.round(totalWriteTime),
          p50: Number(p50(writeLatencies).toFixed(3)),
          p95: Number(p95(writeLatencies).toFixed(3)),
          p99: Number(p99(writeLatencies).toFixed(3)),
          max: Number(max(writeLatencies).toFixed(3)),
        },
        read: {
          qps: readQps,
          durationMs: Math.round(totalReadTime),
          p50: Number(p50(readLatencies).toFixed(3)),
          p95: Number(p95(readLatencies).toFixed(3)),
          p99: Number(p99(readLatencies).toFixed(3)),
          max: Number(max(readLatencies).toFixed(3)),
        },
        stats,
      };
    } finally {
      this.isBenchmarking = false;
    }
  }

  async resetData() {
    if (this.db) {
      await this.db.close();
      this.db = null;
    }
    if (fs.existsSync(this.config.dataDir)) {
      fs.rmSync(this.config.dataDir, { recursive: true, force: true });
    }
    fs.mkdirSync(this.config.dataDir, { recursive: true });
    this.db = await open(this.config);
    return { success: true };
  }
}

export const titankvService = new TitanKvService();
