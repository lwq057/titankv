import React from 'react';
import { Database, Cpu, Zap, HardDrive, RefreshCw, Trash2, PlusCircle } from 'lucide-react';

interface HeaderProps {
  stats: any;
  loading: boolean;
  onRefresh: () => void;
  onSeed: () => void;
  onReset: () => void;
}

export const Header: React.FC<HeaderProps> = ({ stats, loading, onRefresh, onSeed, onReset }) => {
  return (
    <header className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md sticky top-0 z-50">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className="h-10 w-10 rounded-xl bg-gradient-to-br from-amber-500 to-orange-600 flex items-center justify-center shadow-lg shadow-orange-500/20 ring-1 ring-white/10">
            <Zap className="h-5 w-5 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <span className="font-bold text-lg text-zinc-100 tracking-tight">TitanKV</span>
              <span className="text-[10px] uppercase font-mono px-2 py-0.5 rounded-full bg-orange-500/10 text-orange-400 border border-orange-500/20 font-semibold">
                Rust Core • NAPI-RS
              </span>
            </div>
            <p className="text-xs text-zinc-400">High-Performance Persistent KV Storage Engine</p>
          </div>
        </div>

        <div className="flex items-center space-x-2 sm:space-x-3">
          <button
            onClick={onSeed}
            disabled={loading}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-zinc-800/80 hover:bg-zinc-750 text-zinc-200 border border-zinc-700/60 transition disabled:opacity-50"
            title="Seed sample data"
          >
            <PlusCircle className="h-3.5 w-3.5 text-amber-400" />
            <span className="hidden sm:inline">Seed Demo</span>
          </button>

          <button
            onClick={onReset}
            disabled={loading}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-zinc-800/80 hover:bg-red-950/40 text-zinc-200 hover:text-red-300 border border-zinc-700/60 hover:border-red-800/50 transition disabled:opacity-50"
            title="Clear and reset storage"
          >
            <Trash2 className="h-3.5 w-3.5 text-red-400" />
            <span className="hidden sm:inline">Reset Engine</span>
          </button>

          <button
            onClick={onRefresh}
            disabled={loading}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-orange-500/10 hover:bg-orange-500/20 text-orange-300 border border-orange-500/30 transition disabled:opacity-50"
            title="Refresh statistics"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </button>
        </div>
      </div>
    </header>
  );
};
