import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { open } from '../src/titankv/index.ts';

const BASE_TEST_DIR = path.resolve('/tmp/titankv_unit_tests_' + Date.now());

test.before(() => {
  if (fs.existsSync(BASE_TEST_DIR)) {
    fs.rmSync(BASE_TEST_DIR, { recursive: true, force: true });
  }
  fs.mkdirSync(BASE_TEST_DIR, { recursive: true });
});

test.after(() => {
  if (fs.existsSync(BASE_TEST_DIR)) {
    fs.rmSync(BASE_TEST_DIR, { recursive: true, force: true });
  }
});

test('TitanKV: open, put, get, and delete basic workflow', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_basic');
  const db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  const key = 'user:1001';
  const jsonPayload = JSON.stringify({ id: 1001, name: 'Alice', role: 'admin' });
  const buf = Buffer.from(jsonPayload);

  // Put
  const putOk = await db.put(key, buf);
  assert.equal(putOk, true);

  // Get
  const retrieved = (await db.get(key)) as Buffer;
  assert.ok(retrieved, 'retrieved should not be null');
  assert.equal(retrieved.toString(), jsonPayload);

  // Delete
  const delOk = await db.delete(key);
  assert.equal(delOk, true);

  // Verify deleted
  const afterDelete = await db.get(key);
  assert.equal(afterDelete, null);

  await db.close();
});

test('TitanKV: LZ4 compression on large JSON cache (>2KB)', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_lz4');
  const db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
    compression: {
      enabled: true,
      minBytes: 2048,
      minRatio: 0.9,
    },
  });

  // Generate 8KB repetitive JSON cache
  const largeObject = {
    title: 'High-Performance Rust Storage Engine',
    items: Array.from({ length: 150 }, (_, i) => ({
      index: i,
      uuid: 'c87140b9-52e8-4669-9069-1c9f7cb5' + i,
      timestamp: 1731000000 + i,
      status: 'active',
    })),
  };
  const jsonStr = JSON.stringify(largeObject);
  const largeBuffer = Buffer.from(jsonStr);
  assert.ok(largeBuffer.length > 4000);

  await db.put('cache:large:1', largeBuffer);
  const retrieved = (await db.get('cache:large:1')) as Buffer;
  assert.ok(retrieved);
  assert.equal(retrieved.length, largeBuffer.length);
  assert.equal(retrieved.toString(), jsonStr);

  await db.close();
});

test('TitanKV: overwrite, skip, and ttl_compare modes', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_modes');
  const db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  const key = 'item:mode:test';
  await db.put(key, Buffer.from('v1'), { ttl: 100000 });

  // Mode: skip -> should not overwrite existing valid key
  const skipped = await db.put(key, Buffer.from('v2_skipped'), { mode: 'skip' });
  assert.equal(skipped, false);
  let res = (await db.get(key)) as Buffer;
  assert.equal(res.toString(), 'v1');

  // Mode: ttl_compare -> require newExpire - oldExpire >= ttlDiff
  // Old ttl is ~100000ms. If we set with ttl=105000 and ttlDiff=10000, diff is only ~5000ms -> should skip!
  const ttlComparedFail = await db.put(key, Buffer.from('v3_failed'), {
    mode: 'ttl_compare',
    ttl: 105000,
    ttlDiff: 10000,
  });
  assert.equal(ttlComparedFail, false);
  res = (await db.get(key)) as Buffer;
  assert.equal(res.toString(), 'v1');

  // If we set with ttl=125000 and ttlDiff=10000, diff is ~25000ms -> should overwrite!
  const ttlComparedSuccess = await db.put(key, Buffer.from('v4_success'), {
    mode: 'ttl_compare',
    ttl: 125000,
    ttlDiff: 10000,
  });
  assert.equal(ttlComparedSuccess, true);
  res = (await db.get(key)) as Buffer;
  assert.equal(res.toString(), 'v4_success');

  // Mode: overwrite
  const overwrote = await db.put(key, Buffer.from('v5_overwrite'), { mode: 'overwrite' });
  assert.equal(overwrote, true);
  res = (await db.get(key)) as Buffer;
  assert.equal(res.toString(), 'v5_overwrite');

  await db.close();
});

test('TitanKV: TTL query, increase, and reduce operations', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_ttl');
  const db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  const key = 'key:ttl:ops';
  await db.put(key, Buffer.from('data'), { ttl: 60000 });

  // Query TTL
  const initialTtl = await db.ttl(key);
  assert.ok(initialTtl);
  assert.ok(initialTtl.ttl > 50000 && initialTtl.ttl <= 60000);
  assert.ok(initialTtl.expireAt > 0);

  // Increase TTL by 30000ms
  const increased = await db.ttl(key, { increase: 30000 });
  assert.ok(increased);
  assert.ok(increased.ttl > 80000);

  // Reduce TTL by 20000ms
  const reduced = await db.ttl(key, { reduce: 20000 });
  assert.ok(reduced);
  assert.ok(reduced.ttl > 60000 && reduced.ttl <= 75000);

  // Get with withTTL and withExpireAt metadata flags
  const metaRes = (await db.get(key, { withTTL: true, withExpireAt: true })) as any;
  assert.ok(metaRes);
  assert.ok(metaRes.value);
  assert.equal(metaRes.value.toString(), 'data');
  assert.ok(typeof metaRes.ttl === 'number');
  assert.ok(typeof metaRes.expireAt === 'number');

  await db.close();
});

test('TitanKV: Batch operations (putMany, getMany, deleteMany, ttlMany)', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_batch');
  const db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  const items = [
    { key: 'batch:1', value: Buffer.from('val1'), options: { ttl: 50000 } },
    { key: 'batch:2', value: Buffer.from('val2'), options: { ttl: 50000 } },
    { key: 'batch:3', value: Buffer.from('val3'), options: { ttl: 50000 } },
  ];

  // putMany
  const putResults = await db.putMany(items);
  assert.equal(putResults.length, 3);
  assert.ok(putResults.every((r) => r.success));

  // getMany
  const getResults = await db.getMany(['batch:1', 'batch:2', 'batch:3', 'batch:nonexistent']);
  assert.equal(getResults.length, 4);
  assert.equal((getResults[0] as Buffer).toString(), 'val1');
  assert.equal((getResults[1] as Buffer).toString(), 'val2');
  assert.equal((getResults[2] as Buffer).toString(), 'val3');
  assert.equal(getResults[3], null);

  // ttlMany
  const ttlResults = await db.ttlMany([
    { key: 'batch:1', increase: 10000 },
    { key: 'batch:2', reduce: 10000 },
  ]);
  assert.equal(ttlResults.length, 2);
  assert.ok(ttlResults[0]);
  assert.ok(ttlResults[1]);

  // deleteMany
  const delResults = await db.deleteMany(['batch:1', 'batch:2']);
  assert.equal(delResults.length, 2);
  assert.equal(delResults[0], true);
  assert.equal(delResults[1], true);

  const finalCheck = await db.getMany(['batch:1', 'batch:2', 'batch:3']);
  assert.equal(finalCheck[0], null);
  assert.equal(finalCheck[1], null);
  assert.equal((finalCheck[2] as Buffer).toString(), 'val3');

  await db.close();
});

test('TitanKV: Crash recovery and log replay persistence', async () => {
  const dir = path.join(BASE_TEST_DIR, 'test_recovery');

  // Step 1: Open, write records
  let db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  await db.put('persist:k1', Buffer.from('persistent_value_1'));
  await db.put('persist:k2', Buffer.from('persistent_value_2'));
  await db.put('persist:k3', Buffer.from('to_be_deleted'));
  await db.delete('persist:k3');

  // Close to simulate process termination
  await db.close();

  // Step 2: Reopen with same dataDir -> triggers automatic log replay & index reconstruction
  db = await open({
    dataDir: dir,
    global_shardCount: 16,
    global_bucketCount: 1024,
  });

  const k1 = (await db.get('persist:k1')) as Buffer;
  const k2 = (await db.get('persist:k2')) as Buffer;
  const k3 = await db.get('persist:k3');

  assert.ok(k1, 'persist:k1 should exist after recovery');
  assert.equal(k1.toString(), 'persistent_value_1');
  assert.ok(k2, 'persist:k2 should exist after recovery');
  assert.equal(k2.toString(), 'persistent_value_2');
  assert.equal(k3, null, 'persist:k3 should be deleted');

  await db.close();
});
