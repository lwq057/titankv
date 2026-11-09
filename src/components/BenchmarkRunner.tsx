import React, { useState } from 'react';
import { Gauge, Zap, TrendingUp, Activity, BarChart3, ShieldCheck, Flame, RefreshCw } from 'lucide-react';

interface BenchmarkRunnerProps {
  onBenchmarkDone?: () => void;
}

export const BenchmarkRunner: React.FC<BenchmarkRunnerProps> = ({ onBenchmarkDone }) => {
  const [totalKeys, setTotalKeys] = useState(5000);
  const [batchSize, setBatchSize] = useState(50);
  const [payloadSize, setPayloadSize] = useState(512);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const handleRun = async () => {
    setRunning(true);
    setError(null);
    try {
      const res = await fetch('/api/titankv/benchmark', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          totalKeys,
          batchSize,
          payloadSize,
        }),
      });
      const data = await res.json();
      if (!data.success) throw new Error(data.error);
      setResult(data.data);
      onBenchmarkDone?.();
    } catch (err: any) {
      setError(err.message || 'Benchmark execution failed');
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-2xl p-5 mb-8 shadow-sm">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-zinc-800 gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Flame className="h-5 w-5 text-orange-500" />
            <h2 className="text-base font-semibold text-zinc-100">Native Concurrency & Throughput Benchmark</h2>
          </div>
          <p className="text-xs text-zinc-400">
            Stress-tests the 64-lane append-only Value Log and 65,536-stripe RW locks in real-time
          </p>
        </div>

        <button
          onClick={handleRun}
          disabled={running}
          className="flex items-center justify-center gap-2 px-5 py-2 rounded-xl text-xs font-semibold bg-gradient-to-r from-orange-500 to-amber-500 hover:from-orange-400 hover:to-amber-400 text-white shadow-lg shadow-orange-500/20 transition disabled:opacity-50 self-start sm:self-auto cursor-pointer"
        >
          {running ? (
            <>
              <RefreshCw className="h-4 w-4 animate-spin" />
              <span>Running Benchmark...</span>
            </>
          ) : (
            <>
              <Zap className="h-4 w-4 fill-current" />
              <span>Launch Stress Test</span>
            </>
          )}
        </button>
      </div>

      {/* Dials & Parameters */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 my-5">
        <div>
          <div className="flex justify-between text-xs mb-1.5">
            <span className="text-zinc-400">Total Key Operations</span>
            <span className="font-mono text-zinc-200 font-semibold">{totalKeys.toLocaleString()} keys</span>
          </div>
          <div className="flex gap-2">
            {[1000, 5000, 10000, 20000].map((num) => (
              <button
                key={num}
                type="button"
                disabled={running}
                onClick={() => setTotalKeys(num)}
                className={`flex-1 py-1.5 rounded-lg text-xs font-medium border transition ${
                  totalKeys === num
                    ? 'bg-orange-500/20 border-orange-500 text-orange-300'
                    : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                }`}
              >
                {num >= 1000 ? `${num / 1000}k` : num}
              </button>
            ))}
          </div>
        </div>

        <div>
          <div className="flex justify-between text-xs mb-1.5">
            <span className="text-zinc-400">Batch Size (putMany / getMany)</span>
            <span className="font-mono text-zinc-200 font-semibold">{batchSize} items</span>
          </div>
          <div className="flex gap-2">
            {[20, 50, 100, 200].map((num) => (
              <button
                key={num}
                type="button"
                disabled={running}
                onClick={() => setBatchSize(num)}
                className={`flex-1 py-1.5 rounded-lg text-xs font-medium border transition ${
                  batchSize === num
                    ? 'bg-orange-500/20 border-orange-500 text-orange-300'
                    : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                }`}
              >
                {num}
              </button>
            ))}
          </div>
        </div>

        <div>
          <div className="flex justify-between text-xs mb-1.5">
            <span className="text-zinc-400">Payload Size (JSON / Buffer)</span>
            <span className="font-mono text-zinc-200 font-semibold">{payloadSize} B</span>
          </div>
          <div className="flex gap-2">
            {[128, 512, 1024, 4096].map((num) => (
              <button
                key={num}
                type="button"
                disabled={running}
                onClick={() => setPayloadSize(num)}
                className={`flex-1 py-1.5 rounded-lg text-xs font-medium border transition ${
                  payloadSize === num
                    ? 'bg-orange-500/20 border-orange-500 text-orange-300'
                    : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                }`}
              >
                {num >= 1024 ? `${num / 1024}KB` : `${num}B`}
              </button>
            ))}
          </div>
        </div>
      </div>

      {error && (
        <div className="p-3 mb-4 rounded-xl bg-red-950/30 border border-red-800/40 text-xs text-red-300">
          {error}
        </div>
      )}

      {/* Results View */}
      {result ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
          {/* Write Throughput Card */}
          <div className="bg-zinc-950 border border-zinc-800/90 rounded-xl p-4 flex flex-col justify-between">
            <div>
              <div className="flex items-center justify-between mb-3">
                <span className="text-xs font-semibold uppercase tracking-wider text-orange-400 flex items-center gap-1.5">
                  <TrendingUp className="h-4 w-4" />
                  Append-Only Value Log Writes
                </span>
                <span className="text-xs font-mono text-zinc-400">{result.write.durationMs} ms total</span>
              </div>
              <div className="flex items-baseline gap-2 mb-4">
                <span className="text-3xl font-extrabold font-mono text-white tracking-tight">
                  {result.write.qps.toLocaleString()}
                </span>
                <span className="text-xs text-zinc-400 font-medium">OPS / SEC</span>
              </div>
            </div>

            {/* Latency percentiles */}
            <div className="grid grid-cols-4 gap-2 pt-3 border-t border-zinc-850">
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P50</div>
                <div className="text-xs font-mono font-bold text-zinc-200">{result.write.p50} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P95</div>
                <div className="text-xs font-mono font-bold text-zinc-200">{result.write.p95} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P99</div>
                <div className="text-xs font-mono font-bold text-amber-400">{result.write.p99} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">MAX</div>
                <div className="text-xs font-mono font-bold text-zinc-400">{result.write.max} ms</div>
              </div>
            </div>
          </div>

          {/* Read Throughput Card */}
          <div className="bg-zinc-950 border border-zinc-800/90 rounded-xl p-4 flex flex-col justify-between">
            <div>
              <div className="flex items-center justify-between mb-3">
                <span className="text-xs font-semibold uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
                  <Activity className="h-4 w-4" />
                  Hash Bucket Direct Reads
                </span>
                <span className="text-xs font-mono text-zinc-400">{result.read.durationMs} ms total</span>
              </div>
              <div className="flex items-baseline gap-2 mb-4">
                <span className="text-3xl font-extrabold font-mono text-white tracking-tight">
                  {result.read.qps.toLocaleString()}
                </span>
                <span className="text-xs text-zinc-400 font-medium">OPS / SEC</span>
              </div>
            </div>

            {/* Latency percentiles */}
            <div className="grid grid-cols-4 gap-2 pt-3 border-t border-zinc-850">
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P50</div>
                <div className="text-xs font-mono font-bold text-zinc-200">{result.read.p50} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P95</div>
                <div className="text-xs font-mono font-bold text-zinc-200">{result.read.p95} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">P99</div>
                <div className="text-xs font-mono font-bold text-emerald-400">{result.read.p99} ms</div>
              </div>
              <div className="text-center bg-zinc-900/60 py-2 rounded-lg border border-zinc-800/60">
                <div className="text-[10px] text-zinc-400 font-medium">MAX</div>
                <div className="text-xs font-mono font-bold text-zinc-400">{result.read.max} ms</div>
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div className="py-8 border border-dashed border-zinc-800 rounded-xl flex flex-col items-center justify-center text-center text-zinc-500">
          <BarChart3 className="h-8 w-8 text-zinc-600 mb-2" />
          <p className="text-xs font-medium text-zinc-400">Click &quot;Launch Stress Test&quot; above to measure native IOPS and latency.</p>
          <p className="text-[11px] text-zinc-500 mt-0.5">Executes multi-lane concurrent batches directly on Rust worker pools</p>
        </div>
      )}
    </div>
  );
};
