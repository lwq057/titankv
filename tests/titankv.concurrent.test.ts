import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { open } from '../src/titankv/index.ts';

const BENCH_DIR = path.resolve('/tmp/titankv_bench_' + Date.now());

test.before(() => {
  if (fs.existsSync(BENCH_DIR)) {
    fs.rmSync(BENCH_DIR, { recursive: true, force: true });
  }
  fs.mkdirSync(BENCH_DIR, { recursive: true });
});

test.after(() => {
  if (fs.existsSync(BENCH_DIR)) {
    fs.rmSync(BENCH_DIR, { recursive: true, force: true });
  }
});

test('TitanKV: High-Concurrency Multi-Key Read/Write Throughput', async () => {
  const db = await open({
    dataDir: BENCH_DIR,
    global_shardCount: 16,
    global_bucketCount: 4096,
  });

  const TOTAL_KEYS = 2000;
  const BATCH_SIZE = 50;

  // 1. Parallel batched writes
  const startWrite = Date.now();
  const writeBatches: Promise<any>[] = [];

  for (let b = 0; b < TOTAL_KEYS / BATCH_SIZE; b++) {
    const items = [];
    for (let i = 0; i < BATCH_SIZE; i++) {
      const idx = b * BATCH_SIZE + i;
      items.push({
        key: `bench:user:${idx}`,
        value: Buffer.from(JSON.stringify({ id: idx, score: idx * 10, ts: Date.now() })),
        options: { ttl: 3600000 },
      });
    }
    writeBatches.push(db.putMany(items));
  }

  await Promise.all(writeBatches);
  const writeDuration = Date.now() - startWrite;
  const writeQps = Math.round((TOTAL_KEYS / (writeDuration || 1)) * 1000);
  console.log(`[Bench] Parallel Write: ${TOTAL_KEYS} keys in ${writeDuration}ms (~${writeQps} ops/sec)`);

  // 2. Parallel batched reads
  const startRead = Date.now();
  const readBatches: Promise<any>[] = [];
  for (let b = 0; b < TOTAL_KEYS / BATCH_SIZE; b++) {
    const keys = [];
    for (let i = 0; i < BATCH_SIZE; i++) {
      const idx = b * BATCH_SIZE + i;
      keys.push(`bench:user:${idx}`);
    }
    readBatches.push(db.getMany(keys));
  }

  const readResults = await Promise.all(readBatches);
  const readDuration = Date.now() - startRead;
  const readQps = Math.round((TOTAL_KEYS / (readDuration || 1)) * 1000);
  console.log(`[Bench] Parallel Read: ${TOTAL_KEYS} keys in ${readDuration}ms (~${readQps} ops/sec)`);

  // Verify integrity
  let verifiedCount = 0;
  for (const batch of readResults) {
    for (const val of batch) {
      if (val) {
        verifiedCount++;
      }
    }
  }
  assert.equal(verifiedCount, TOTAL_KEYS, 'All keys should be successfully retrieved');

  // Check stats
  const stats = await db.stats();
  assert.ok(stats.total_keys >= TOTAL_KEYS);
  console.log('[Bench] Engine Stats:', stats);

  await db.close();
});
