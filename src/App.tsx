import React, { useState, useEffect, useCallback } from 'react';
import { Header } from './components/Header.tsx';
import { MetricsOverview } from './components/MetricsOverview.tsx';
import { Workbench } from './components/Workbench.tsx';
import { BenchmarkRunner } from './components/BenchmarkRunner.tsx';
import { ArchitectureView } from './components/ArchitectureView.tsx';
import { ApiReference } from './components/ApiReference.tsx';
import { ShieldCheck, Terminal, Cpu, HardDrive } from 'lucide-react';

export default function App() {
  const [stats, setStats] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [toastMsg, setToastMsg] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToastMsg(msg);
    setTimeout(() => setToastMsg(null), 3000);
  };

  const fetchStats = useCallback(async () => {
    try {
      const res = await fetch('/api/titankv/stats');
      const data = await res.json();
      if (data.success) {
        setStats(data.data);
      }
    } catch (e) {
      console.error('Failed to fetch stats:', e);
    }
  }, []);

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 2500);
    return () => clearInterval(interval);
  }, [fetchStats]);

  const handleRefresh = async () => {
    setLoading(true);
    await fetchStats();
    setLoading(false);
    showToast('Engine telemetry updated');
  };

  const handleSeed = async () => {
    setLoading(true);
    try {
      const res = await fetch('/api/titankv/seed-sample', { method: 'POST' });
      const data = await res.json();
      if (data.success) {
        await fetchStats();
        showToast(`Seeded ${data.seeded} production sample records!`);
      }
    } catch {
      showToast('Failed to seed records');
    } finally {
      setLoading(false);
    }
  };

  const handleReset = async () => {
    if (!confirm('Are you sure you want to reset and purge the TitanKV storage directory?')) {
      return;
    }
    setLoading(true);
    try {
      const res = await fetch('/api/titankv/reset', { method: 'POST' });
      const data = await res.json();
      if (data.success) {
        await fetchStats();
        showToast('Storage purged and reinitialized!');
      }
    } catch {
      showToast('Failed to reset storage');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 selection:bg-orange-500 selection:text-white font-sans antialiased">
      {/* Toast popup */}
      {toastMsg && (
        <div className="fixed bottom-5 right-5 z-50 bg-orange-600 text-white text-xs font-semibold px-4 py-2.5 rounded-xl shadow-xl shadow-orange-600/30 flex items-center gap-2 animate-bounce">
          <ShieldCheck className="h-4 w-4" />
          <span>{toastMsg}</span>
        </div>
      )}

      {/* Top Navbar */}
      <Header
        stats={stats}
        loading={loading}
        onRefresh={handleRefresh}
        onSeed={handleSeed}
        onReset={handleReset}
      />

      {/* Main Content Area */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {/* Hero Section */}
        <div className="mb-6">
          <h1 className="text-xl sm:text-2xl font-extrabold text-white tracking-tight flex items-center gap-2">
            <span>TitanKV Storage Engine Dashboard</span>
          </h1>
          <p className="text-xs sm:text-sm text-zinc-400 mt-1 max-w-3xl">
            Ultra-fast, zero-dependency persistent KV engine written in Rust with NAPI-RS Node.js bindings.
            Featuring 4KB Fixed Shard Hash Bucket Pages, 64-lane Append-Only Value Log, OFD process locks, and Bounded Min-Heap TTL.
          </p>
        </div>

        {/* Real-time telemetry cards */}
        <MetricsOverview stats={stats} />

        {/* Interactive Workbench */}
        <Workbench onSuccessAction={fetchStats} />

        {/* High-throughput Benchmark Suite */}
        <BenchmarkRunner onBenchmarkDone={fetchStats} />

        {/* Technical Architecture Breakdown */}
        <ArchitectureView />

        {/* Node.js Native SDK Reference */}
        <ApiReference />

        {/* Footer */}
        <footer className="pt-6 pb-12 border-t border-zinc-850 text-center text-xs text-zinc-400">
          <div className="flex items-center justify-center gap-2 mb-1 text-zinc-300 font-medium">
            <span>TitanKV Core v1.0.0</span>
            <span>•</span>
            <span className="font-mono text-orange-400">titankv.linux-x64-gnu.node</span>
            <span>•</span>
            <span>Rust 2021 Edition</span>
          </div>
          <p className="text-[11px] text-zinc-400">
            Engineered for 100M+ keys with SSD hardware block alignment, zero-copy buffer transfers, and memory-bounded stability.
          </p>
        </footer>
      </main>
    </div>
  );
}
