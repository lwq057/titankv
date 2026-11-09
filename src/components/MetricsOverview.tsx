import React from 'react';
import { Database, HardDrive, Cpu, Clock, Layers, Sparkles } from 'lucide-react';

interface MetricsOverviewProps {
  stats: any;
}

export const MetricsOverview: React.FC<MetricsOverviewProps> = ({ stats }) => {
  const formatBytes = (bytes: number) => {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const totalKeys = stats?.total_keys ?? stats?.totalKeys ?? 0;
  const memUsed = stats?.memory_used_bytes ?? stats?.memoryUsedBytes ?? 0;
  const diskBytes = stats?.totalDiskBytes ?? 0;
  const liveBytes = stats?.live_bytes ?? stats?.liveBytes ?? 0;
  const ttlCleaned = stats?.ttl_cleaned_count ?? stats?.ttlCleanedCount ?? 0;
  const segments = stats?.segments_count ?? stats?.segmentsCount ?? 0;
  const shards = stats?.shards ?? 16;
  const lanes = stats?.writerLanes ?? 64;

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3 sm:gap-4 mb-6">
      {/* Metric 1: Total Keys */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">Live Keys</span>
          <Database className="h-4 w-4 text-orange-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {totalKeys.toLocaleString()}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1 flex items-center gap-1">
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
            <span>Active in Shards</span>
          </div>
        </div>
      </div>

      {/* Metric 2: Live Payload */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">Payload Data</span>
          <Layers className="h-4 w-4 text-amber-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {formatBytes(liveBytes)}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1">Uncompressed size</div>
        </div>
      </div>

      {/* Metric 3: Memory Budget */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">Memory Budget</span>
          <Cpu className="h-4 w-4 text-sky-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {formatBytes(memUsed)}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1">Pools & Queues (512MB max)</div>
        </div>
      </div>

      {/* Metric 4: Disk Storage */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">Disk Footprint</span>
          <HardDrive className="h-4 w-4 text-indigo-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {formatBytes(diskBytes)}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1">Logs & Overflow pages</div>
        </div>
      </div>

      {/* Metric 5: Shards & Lanes */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">Shards / Lanes</span>
          <Layers className="h-4 w-4 text-violet-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {shards} / {lanes}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1">Striped RW lock routing</div>
        </div>
      </div>

      {/* Metric 6: TTL Cleaned */}
      <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-xl p-4 flex flex-col justify-between hover:border-zinc-700/80 transition shadow-sm">
        <div className="flex items-center justify-between text-zinc-400 mb-2">
          <span className="text-xs font-medium">TTL Cleaned</span>
          <Clock className="h-4 w-4 text-emerald-400" />
        </div>
        <div>
          <div className="text-2xl font-bold font-mono text-zinc-100 tracking-tight">
            {ttlCleaned.toLocaleString()}
          </div>
          <div className="text-[11px] text-zinc-400 mt-1">Min-Heap background swept</div>
        </div>
      </div>
    </div>
  );
};
