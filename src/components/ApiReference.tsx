import React, { useState } from 'react';
import { Code2, Copy, Check, Terminal, ExternalLink } from 'lucide-react';

export const ApiReference: React.FC = () => {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const snippets = [
    {
      title: 'Initialize Engine (open)',
      description: 'Open or create a TitanKV storage directory with custom configuration',
      code: `import { open } from './src/titankv/index.ts';

const db = await open({
  dataDir: './my_titankv_data',
  global_shardCount: 16,
  global_bucketCount: 4194304, // 4M primary buckets
  compression: {
    enabled: true,
    minBytes: 2048,
    minRatio: 0.9,
  },
  memory: {
    maxBytes: 512 * 1024 * 1024, // 512MB RAM budget
  },
  ttl: {
    enabled: true,
    heapCapacity: 50000,
  },
});`,
    },
    {
      title: 'Write & Overwrite Modes (put, putMany)',
      description: 'Zero-copy write supporting raw Buffers, TTLs, and conditional modes',
      code: `// Direct PUT with 10-minute TTL
await db.put('user:1001', Buffer.from(JSON.stringify({ name: 'Alice', rank: 1 })), {
  ttl: 600000,
});

// Conditional Overwrite (ttl_compare mode)
// Overwrites only if (newExpireAt - oldExpireAt) >= ttlDiff
await db.put('session:cache:98', Buffer.from('payload'), {
  mode: 'ttl_compare',
  ttl: 3600000,
  ttlDiff: 600000,
});

// Batch PUT
await db.putMany([
  { key: 'item:1', value: Buffer.from('val1'), options: { ttl: 60000 } },
  { key: 'item:2', value: Buffer.from('val2'), options: { mode: 'skip' } },
]);`,
    },
    {
      title: 'Read with Metadata (get, getMany)',
      description: 'Direct zero-copy read with TTL and absolute expiration timestamps',
      code: `// Basic GET
const buf = await db.get('user:1001'); // Returns Buffer | null

// GET with TTL and ExpireAt metadata flags
const res = await db.get('user:1001', {
  withTTL: true,
  withExpireAt: true,
  allowExpired: false,
});
console.log(res.value.toString()); // Buffer
console.log(res.ttl);             // Remaining ms
console.log(res.expireAt);        // Epoch timestamp ms

// Batch GET
const results = await db.getMany(['user:1001', 'user:1002', 'user:1003']);`,
    },
    {
      title: 'TTL Manipulation (ttl, ttlMany)',
      description: 'Query, update, increase or reduce TTL atomically without touching value',
      code: `// Query remaining TTL
const info = await db.ttl('user:1001');
console.log(info.ttl, info.expireAt);

// Atomically extend TTL by 30 seconds
await db.ttl('user:1001', { increase: 30000 });

// Atomically reduce TTL by 10 seconds
await db.ttl('user:1001', { reduce: 10000 });

// Batch TTL updates
await db.ttlMany([
  { key: 'user:1001', increase: 60000 },
  { key: 'user:1002', reduce: 15000 },
]);`,
    },
  ];

  const handleCopy = (text: string, index: number) => {
    navigator.clipboard.writeText(text);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 2000);
  };

  return (
    <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-2xl p-5 mb-8 shadow-sm">
      <div className="flex items-center gap-2 pb-4 border-b border-zinc-800 mb-5">
        <Code2 className="h-5 w-5 text-orange-400" />
        <div>
          <h2 className="text-base font-semibold text-zinc-100">Node.js Native SDK Reference</h2>
          <p className="text-xs text-zinc-400">Complete API documentation and zero-copy code patterns</p>
        </div>
      </div>

      <div className="space-y-4">
        {snippets.map((snip, idx) => (
          <div key={idx} className="bg-zinc-950 border border-zinc-800 rounded-xl overflow-hidden">
            <div className="px-4 py-3 bg-zinc-900/60 border-b border-zinc-850 flex items-center justify-between">
              <div>
                <span className="text-xs font-semibold text-zinc-200">{snip.title}</span>
                <p className="text-[11px] text-zinc-400">{snip.description}</p>
              </div>
              <button
                onClick={() => handleCopy(snip.code, idx)}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition"
              >
                {copiedIndex === idx ? (
                  <>
                    <Check className="h-3 w-3 text-emerald-400" />
                    <span className="text-emerald-400">Copied</span>
                  </>
                ) : (
                  <>
                    <Copy className="h-3 w-3 text-zinc-400" />
                    <span>Copy</span>
                  </>
                )}
              </button>
            </div>
            <pre className="p-4 text-xs font-mono text-zinc-300 overflow-x-auto leading-relaxed">
              <code>{snip.code}</code>
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
};
