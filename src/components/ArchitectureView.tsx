import React, { useState } from 'react';
import { Layers, HardDrive, ShieldCheck, Clock, Cpu, Lock, CheckCircle2, ChevronRight } from 'lucide-react';

export const ArchitectureView: React.FC = () => {
  const [selectedPillar, setSelectedPillar] = useState(0);

  const pillars = [
    {
      id: 'hash_index',
      title: 'Fixed Shard Hash Index',
      subtitle: '4KB Bucket Pages & Slot Directory',
      icon: Layers,
      color: 'from-orange-500 to-amber-600',
      badge: 'Index Engine',
      description:
        'Eliminates LSM-tree read/write amplification. Direct address mapping splits keys across 16 fixed shards with 4,194,304 primary buckets. If a bucket exceeds 4KB, an overflow page is allocated and chained via 64-bit pointers.',
      highlights: [
        '4KB page alignment matching SSD hardware physical block size',
        'Slot directory binary search with 64-bit hash prefix filter',
        'Zero page rewrite: append-only chained overflow pages',
        'Direct pointer mapping: (segment_id, offset, stored_len)',
      ],
      details: {
        bucketFormat: 'Header (magic, record_count, next_page_id) + Slot Directory + Variable Records',
        hashAlgorithm: 'xxHash64 / CityHash with configurable seed',
        concurrency: 'Read-shared / Write-exclusive lock per bucket',
      },
    },
    {
      id: 'val_log',
      title: 'Append-Only Value Log',
      subtitle: 'Multi-Lane Rolling Segments & LZ4',
      icon: HardDrive,
      color: 'from-blue-500 to-indigo-600',
      badge: 'Persistence Log',
      description:
        'All write operations (PUT, DELETE tombstones, and TTL updates) are serialized sequentially into rolling segments. By splitting writers into 64 dedicated lanes, multi-process writers never block each other.',
      highlights: [
        'Rolling 256MB/64MB segments with monotonic sequence numbers',
        'Framing with CRC32C integrity checksum and TEND commit markers',
        'LZ4 compression active on values > 2KB with < 0.9 ratio guard',
        'Tombstone markers for garbage collection (Segment GC)',
      ],
      details: {
        recordHeader: 'TLOG (4B) | Type (1B) | Flags (2B) | Seq (8B) | ExpireAt (8B) | Gen (4B)',
        commitMarker: 'TEND (4B) verifying uninterrupted sequential disk commit',
        compression: 'lz4_flex zero-allocation compression engine',
      },
    },
    {
      id: 'process_locks',
      title: 'Multi-Process Concurrency',
      subtitle: 'Striped RW Locks & OFD Byte Locks',
      icon: Lock,
      color: 'from-emerald-500 to-teal-600',
      badge: 'Lock Safety',
      description:
        'Full multi-process safety allowing multiple Node.js cluster workers to read and write to the same storage directory simultaneously without socket daemon bottlenecks.',
      highlights: [
        'Tier 1: 65,536 striped local in-process parking_lot RW locks',
        'Tier 2: Linux OFD (Open File Description) byte-range lock hierarchy',
        'Strict lock ordering: Local Lock -> Process Lock prevents deadlock',
        'Zero inter-process socket serialization overhead',
      ],
      details: {
        bucketLock: 'locks/bucket.lock (byte offset = global_bucket_id)',
        databaseLock: 'locks/database.lock (lifecycle coordination)',
        recoveryLock: 'locks/recovery.lock (ensures primary replay leader)',
      },
    },
    {
      id: 'ttl_engine',
      title: 'Independent Key TTL',
      subtitle: 'Bounded Min-Heap & Disk Buckets',
      icon: Clock,
      color: 'from-violet-500 to-purple-600',
      badge: 'TTL Automation',
      description:
        'Supports individual TTL per key. A memory-bounded Min-Heap keeps track of the earliest expiring keys, while secondary events are persisted in time-partitioned disk buckets.',
      highlights: [
        'Dual-phase TTL cleanup: Lazy check on read + background Min-Heap sweeper',
        'Disk time buckets partition expiring keys into 60-second windows',
        'Atomic TTL operations: query, overwrite, increase (+ms), reduce (-ms)',
        'ttl_compare mode: overwrite conditionally on TTL difference threshold',
      ],
      details: {
        heapCapacity: '50,000 keys bounded to prevent RAM exhaustion',
        prefetching: 'Loads upcoming window from disk into memory heap',
        tombstoneSweep: 'Generates sequence tombstone when cleaned',
      },
    },
    {
      id: 'memory_budget',
      title: 'Zero-Copy & Low Memory',
      subtitle: 'Budget Enforcement & Buffer Pools',
      icon: Cpu,
      color: 'from-amber-500 to-red-600',
      badge: 'Zero-Copy NAPI',
      description:
        'Designed to run reliably with bounded RAM usage (<512MB default). Values are kept on disk and read directly into Node.js Buffer instances with zero double-buffering.',
      highlights: [
        'Thread-safe memory budget tracker: rejects writes if budget exceeded',
        'Pooled buffer allocators: 4KB, 16KB, 64KB, 128KB slices',
        'Raw byte buffer storage without Rust JSON parsing overhead',
        'napi-rs non-blocking async execution off the libuv threadpool',
      ],
      details: {
        budgetLimit: 'Configurable (default: 512MB max resident memory)',
        queueBudget: '64MB queue buffer limit',
        inFlightBudget: '64MB concurrent in-flight request limit',
      },
    },
  ];

  const current = pillars[selectedPillar];
  const Icon = current.icon;

  return (
    <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-2xl p-5 mb-8 shadow-sm">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-zinc-800 gap-2 mb-5">
        <div>
          <h2 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
            <Layers className="h-4 w-4 text-orange-400" />
            <span>TitanKV High-Performance Storage Architecture</span>
          </h2>
          <p className="text-xs text-zinc-400">
            Engineered according to specification V1.0 for sub-millisecond IOPS and 100M+ key density
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-2 mb-5">
        {pillars.map((p, idx) => {
          const PIcon = p.icon;
          const isSelected = selectedPillar === idx;
          return (
            <button
              key={p.id}
              onClick={() => setSelectedPillar(idx)}
              className={`p-3 rounded-xl border text-left transition flex flex-col justify-between cursor-pointer ${
                isSelected
                  ? 'bg-zinc-950 border-orange-500/60 shadow-md ring-1 ring-orange-500/20'
                  : 'bg-zinc-950/40 border-zinc-850 hover:border-zinc-700/60 text-zinc-400'
              }`}
            >
              <div className="flex items-center justify-between mb-2">
                <div
                  className={`h-7 w-7 rounded-lg flex items-center justify-center bg-gradient-to-br ${p.color} text-white shadow-sm`}
                >
                  <PIcon className="h-3.5 w-3.5" />
                </div>
                {isSelected && <span className="h-2 w-2 rounded-full bg-orange-400"></span>}
              </div>
              <div>
                <div
                  className={`text-xs font-semibold ${isSelected ? 'text-zinc-100' : 'text-zinc-300'}`}
                >
                  {p.title}
                </div>
                <div className="text-[10px] text-zinc-400 line-clamp-1 mt-0.5">{p.subtitle}</div>
              </div>
            </button>
          );
        })}
      </div>

      {/* Selected Pillar Content Card */}
      <div className="bg-zinc-950 border border-zinc-800 rounded-xl p-5">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-3 pb-4 border-b border-zinc-850">
          <div className="flex items-center gap-3">
            <div
              className={`h-10 w-10 rounded-xl flex items-center justify-center bg-gradient-to-br ${current.color} text-white shadow-md`}
            >
              <Icon className="h-5 w-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-bold text-zinc-100">{current.title}</h3>
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-orange-500/10 text-orange-400 border border-orange-500/20 font-mono font-medium">
                  {current.badge}
                </span>
              </div>
              <p className="text-xs text-zinc-400 mt-0.5">{current.subtitle}</p>
            </div>
          </div>
        </div>

        <p className="text-xs text-zinc-300 leading-relaxed my-4">{current.description}</p>

        {/* Highlights & Details Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 pt-3 border-t border-zinc-850/80">
          <div>
            <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400 block mb-2">
              Core Design Highlights
            </span>
            <ul className="space-y-2">
              {current.highlights.map((h, i) => (
                <li key={i} className="text-xs text-zinc-300 flex items-start gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400 shrink-0 mt-0.5" />
                  <span>{h}</span>
                </li>
              ))}
            </ul>
          </div>

          <div className="bg-zinc-900/50 border border-zinc-800/80 rounded-xl p-3.5 space-y-2.5">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-orange-400 block mb-1">
              Internal Format & Specifications
            </span>
            {Object.entries(current.details).map(([k, v]) => (
              <div key={k} className="text-xs">
                <div className="text-[10px] uppercase font-mono text-zinc-400">{k}</div>
                <div className="font-mono text-zinc-200 mt-0.5 text-[11px]">{v}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
