# TitanKV 开发者使用文档与架构指南

TitanKV 是一款专为超高吞吐、低内存占用、多进程并发持久化存储场景设计的单机 Key-Value 存储引擎。底层基于 Rust 实现核心存储引擎与并发控制，并通过 `napi-rs` 提供原生零抽象损失的 Node.js / TypeScript 绑定。

---

## 一、核心架构概览 (Architecture Overview)

TitanKV 摒弃了传统 LSM-Tree 带来的读写放大与多级 Compaction CPU 抖动，采用 **Bitcask 变种 + 4KB 硬件对齐分片哈希索引页 + 追加写 Value Log** 的极简高性能存储模型。

```text
                                  ┌─────────────────────────────┐
                                  │      Node.js Native SDK     │
                                  └──────────────┬──────────────┘
                                                 │
                                                 ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                 TitanKV Core (Rust)                                    │
│                                                                                        │
│   ┌────────────────────────────────┐            ┌──────────────────────────────────┐   │
│   │   Shard Router (xxHash3 / 64)  │            │      Striped Lock Manager        │   │
│   │   Key -> Shard -> Bucket       │            │  65536 Local RWLock + OS OFD Lock│   │
│   └───────────────┬────────────────┘            └─────────────────┬────────────────┘   │
│                   │                                               │                    │
│                   ▼                                               ▼                    │
│   ┌────────────────────────────────┐            ┌──────────────────────────────────┐   │
│   │    4KB Hash Index (SSD 对齐)   │            │   64-Lane Append-Only Value Log  │   │
│   │  Primary Page + 链式溢出页      │───────────▶│  (TLOG 格式: CRC32C + TEND 标记)  │   │
│   │  双指针 IndexRecord (状态/数据) │            │  PUT / TOMBSTONE / TTL_UPDATE    │   │
│   └────────────────────────────────┘            └─────────────────┬────────────────┘   │
│                   │                                               │                    │
│                   ▼                                               ▼                    │
│   ┌────────────────────────────────┐            ┌──────────────────────────────────┐   │
│   │      Bounded Min-Heap TTL      │            │     Segment GC / Relocation      │   │
│   │  有界小顶堆 + 磁盘时间分桶清理 │            │   有效率阈值驱动搬迁 (0.5)        │   │
│   └────────────────────────────────┘            └──────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────────────────────┘
                                                 │
                                                 ▼
                             ┌───────────────────────────────────────┐
                             │    NVMe SSD / Local File System       │
                             │  .idx (4KB Pages)  |  .log (Append)   │
                             └───────────────────────────────────────┘
```

### 1. 架构四大支柱

1. **追加写日志与多车道并发写入（Multi-Lane Value Log）**：
   - 写入时只进行顺序追加（Sequential Append），利用现代 NVMe 固态硬盘最优写入特性。
   - 全局配置 16 个 Shard × 4 个 Writer Lane = **64 个独立写入车道**。并发写入被打散至不同车道，消除单一全局互斥锁瓶颈。

2. **双指针 IndexRecord（状态与数据解耦）**：
   - `state_segment_id`, `state_offset`, `state_relocation_generation`：记录该 Key 最新的操作日志位置（PUT、TOMBSTONE 或 TTL_UPDATE）。
   - `value_segment_id`, `value_offset`, `stored_len`, `raw_len`：直接锁定 Value 原始数据的磁盘物理坐标。
   - **优势**：执行 `db.ttl(key, { ttl })` 更新到期时间时，仅在日志中追加约 50 字节紧凑元数据，**完全不读写、不解压、不重新复制底层的 Value 数据**。

3. **两级分段锁与跨进程协调（Strict Lock Ordering）**：
   - **第一级（进程内）**：65,536 个 Striped `parking_lot::RwLock`，微秒级读写并发。
   - **第二级（进程间）**：Linux 下采用基于文件描述符的开放文件描述锁（OFD Byte-Range Locks，`F_OFD_SETLK`）；Windows 下使用 `LockFileEx`。
   - 严格遵循 `Local Bucket Lock -> OS Bucket Lock` 的单向锁加锁次序，天然免疫死锁。

4. **有界时间分桶与小顶堆 TTL 清理引擎**：
   - 内存维护固定容量的 `Bounded Min-Heap`，优先淘汰最近临期的 Key。
   - 磁盘日志按时间窗口分桶存放 TTL 事件，后台 Cleaner 定期批量清理过期项并写入墓碑（Tombstone），避免过期脏数据复活。

---

## 二、快速上手 (Quick Start)

### 1. 引入与打开数据库

```typescript
import { open, TitanDatabase } from './src/titankv/index.ts';

async function main() {
  // 打开或创建数据库实例
  const db: TitanDatabase = await open({
    dataDir: './titankv_storage',
    global_shardCount: 16,
    global_bucketCount: 1024,
    compression: {
      enabled: true,
      minBytes: 1024,    // 超过 1KB 尝试压缩
      minRatio: 0.9,     // 压缩率优于 90% 才持久化压缩态
    },
    ttl: {
      enabled: true,
      cleanIntervalMs: 1000,
    },
  });

  console.log('TitanKV 数据库就绪');
}
```

---

## 三、核心 API 清单与代码示例

### 1. 写入数据 (`put`)

```typescript
// 1. 基础写入（未指定 TTL 时，默认为永久有效 -1）
await db.put('user:1001:profile', Buffer.from(JSON.stringify({ name: 'Alice', level: 5 })));

// 2. 带 TTL 写入（单位：毫秒）
await db.put('session:token_xyz', Buffer.from('active_user'), {
  ttl: 300000, // 5 分钟后过期
});

// 3. 覆盖写入（未指定 ttl 时，会自动保留该 Key 原先的 expireAt 到期时间）
await db.put('session:token_xyz', Buffer.from('active_user_v2'));

// 4. skip 模式（仅当 Key 不存在或已过期时写入，若已有有效 Key 则跳过返回 false）
const written = await db.put('lock:order:8899', Buffer.from('locked'), {
  mode: 'skip',
  ttl: 10000,
});

// 5. ttl_compare 模式（新 TTL 必须比旧 TTL 晚指定差值毫秒才允许覆盖）
await db.put('lease:master', Buffer.from('node_1'), {
  mode: 'ttl_compare',
  ttl: 60000,
  ttlDiff: 5000, // 新到期时间至少比旧到期时间大 5000ms
});
```

### 2. 读取数据 (`get`)

```typescript
// 1. 简单读取（返回原始 Buffer，不存在或已过期返回 null）
const val = await db.get('user:1001:profile');
if (val) {
  const data = JSON.parse(val.toString('utf-8'));
  console.log('读取到数据:', data);
}

// 2. 携带元数据读取（获取剩余有效时间与具体到期时间戳）
const detail = await db.get('session:token_xyz', {
  withTTL: true,
  withExpireAt: true,
});
if (detail) {
  console.log('Value:', detail.value.toString());
  console.log('剩余 TTL (ms):', detail.ttl);
  console.log('到期毫秒时间戳:', detail.expireAt);
}

// 3. 允许读取已过期但尚未物理清理的数据
const stale = await db.get('session:token_xyz', {
  allowExpired: true,
});
```

### 3. TTL 控制与续期 (`ttl`)

> **核心保障**：调用 `ttl` 只追加一条约 50 字节的 `TTL_UPDATE` 元数据，**绝不会读写或复制修改底层存储的 Value**。

```typescript
// 1. 查询当前 Key 的 TTL 元信息
const ttlInfo = await db.ttl('session:token_xyz');
console.log('剩余 TTL:', ttlInfo?.ttl, '毫秒时间戳:', ttlInfo?.expireAt);

// 2. 延长有效时间（向后延长 60 秒）
await db.ttl('session:token_xyz', {
  increase: 60000,
});

// 3. 缩减有效时间（提前 30 秒）
await db.ttl('session:token_xyz', {
  reduce: 30000,
});

// 4. 重设绝对 TTL
await db.ttl('session:token_xyz', {
  ttl: 120000, // 重新设置为 2 分钟
});
```

### 4. 删除数据 (`delete`)

```typescript
// 写入 Tombstone 墓碑记录，并立即从索引槽中标记删除
const deleted = await db.delete('session:token_xyz');
console.log('删除状态:', deleted);
```

### 5. 批量高性能操作 (`putMany`, `getMany`, `deleteMany`, `ttlMany`)

批量 API 享受内部连续批处理优化，且具有**独立异常隔离机制**，不会因某单条 Key 异常而中断整批执行：

```typescript
// 1. 批量写入
const putResults = await db.putMany([
  { key: 'item:1', value: Buffer.from('v1'), options: { ttl: 60000 } },
  { key: 'item:2', value: Buffer.from('v2') },
  { key: 'item:3', value: Buffer.from('v3') },
]);

// 2. 批量读取（返回顺序与输入数组严格对应）
const getResults = await db.getMany(['item:1', 'item:2', 'item:not_exist']);
// getResults[0] -> Buffer
// getResults[1] -> Buffer
// getResults[2] -> null

// 3. 批量删除
const delResults = await db.deleteMany(['item:1', 'item:2']);

// 4. 批量 TTL 更新
const ttlResults = await db.ttlMany([
  { key: 'item:3', increase: 10000 },
]);
```

### 6. 运行状态监控与关闭 (`stats`, `close`)

```typescript
// 获取实时遥测统计
const stats = await db.stats();
console.log({
  总Key数量: stats.totalKeys,
  活跃段文件数: stats.segmentsCount,
  已用物理内存: `${(stats.memoryUsedBytes / 1024 / 1024).toFixed(2)} MB`,
  TTL清理次数: stats.ttlCleanedCount,
  GC回收字节数: stats.gcReclaimedBytes,
});

// 安全关闭数据库（刷盘当前活动车道的所有数据并释放文件锁）
await db.close();
```

---

## 四、配置选项字典 (Configuration Reference)

| 配置字段 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `dataDir` | `./.titankv_data` | 数据存储持久化根目录 |
| `global_shardCount` | `16` | 全局 Shard 分片数（推荐 16 或 32） |
| `global_bucketCount` | `1024` | 单 Shard 基础哈希桶数量 |
| `global_segmentSizeBytes` | `268435456` (256MB) | 单个 Log Segment 文件滚动阈值 |
| `compression.enabled` | `true` | 是否启用 LZ4 数据压缩 |
| `compression.minBytes` | `1024` (1KB) | 触发压缩检查的最小 Value 字节数 |
| `compression.minRatio` | `0.9` | 压缩比阈值，压缩后小于原大小 90% 时才存储压缩格式 |
| `concurrency.writerLanesPerShard` | `4` | 单 Shard 写入车道数（总车道数 = Shard × Lane） |
| `concurrency.workerThreads` | `4` | 后台清理与任务工作线程数 |
| `ttl.enabled` | `true` | 是否开启后台 TTL 定期清理任务 |
| `ttl.cleanIntervalMs` | `1000` (1秒) | TTL 后台清理检查时间间隔 |
| `ttl.heapCapacity` | `50000` | 内存 Bounded Min-Heap 最大保留项数 |

---

## 五、关键稳定性与多进程安全保证

1. **原子重放与崩溃一致性**：每次写入记录均以 `TLOG` 魔数起首，以 `TEND` 提交标记与 CRC32C 校验结尾。系统重启时会自动跳过断电造成的半写段（Torn Writes），并按全局单调递增的 Sequence 保证数据不回退。
2. **多进程并发保护**：支持跨多个独立 Node.js / 系统进程共同访问同一个 `dataDir`。系统内置 OFD 锁与磁盘物理文件动态探针，杜绝了溢出页覆写及并发写时脏读。
3. **内存边界管控**：索引为磁盘页格式（Page-driven），热点数据采用预分配定额 BufferPool 与 inflight 预算锁，杜绝突发大流量造成的 Node.js / V8 OOM 崩溃。
