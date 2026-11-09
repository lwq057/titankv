import React, { useState } from 'react';
import { Play, ArrowRight, CornerDownLeft, Sparkles, Check, AlertCircle, Copy, Clock, Database, Tag } from 'lucide-react';

interface WorkbenchProps {
  onSuccessAction?: () => void;
}

export const Workbench: React.FC<WorkbenchProps> = ({ onSuccessAction }) => {
  const [activeTab, setActiveTab] = useState<'put' | 'get' | 'delete' | 'ttl'>('put');

  // Input states
  const [key, setKey] = useState('user:profile:1001');
  const [valJson, setValJson] = useState(
    JSON.stringify(
      {
        userId: 1001,
        username: 'cyber_phoenix',
        role: 'lead_architect',
        tags: ['rust', 'systems', 'storage-engine'],
        metadata: {
          cacheHits: 4920,
          compression: 'lz4',
          priority: 'critical',
        },
      },
      null,
      2
    )
  );
  const [mode, setMode] = useState<'overwrite' | 'skip' | 'ttl_compare'>('overwrite');
  const [ttlMs, setTtlMs] = useState<string>('300000'); // 5 minutes
  const [ttlDiff, setTtlDiff] = useState<string>('60000'); // 1 minute
  const [syncWrite, setSyncWrite] = useState(false);

  // Get options
  const [withTTL, setWithTTL] = useState(true);
  const [withExpireAt, setWithExpireAt] = useState(true);
  const [allowExpired, setAllowExpired] = useState(true);

  // TTL update options
  const [ttlOpType, setTtlOpType] = useState<'query' | 'set' | 'increase' | 'reduce'>('query');
  const [ttlDeltaVal, setTtlDeltaVal] = useState('60000');

  // Response state
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<any>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleExecute = async () => {
    setLoading(true);
    setErrorMsg(null);
    setResponse(null);
    const start = performance.now();

    try {
      if (activeTab === 'put') {
        let parsedVal: any = valJson;
        try {
          parsedVal = JSON.parse(valJson);
        } catch {}

        const res = await fetch('/api/titankv/put', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            key,
            value: parsedVal,
            mode,
            ttl: ttlMs ? Number(ttlMs) : undefined,
            ttlDiff: ttlDiff ? Number(ttlDiff) : undefined,
            sync: syncWrite,
          }),
        });
        const data = await res.json();
        if (!data.success) throw new Error(data.error);
        setResponse(data.data);
      } else if (activeTab === 'get') {
        const res = await fetch('/api/titankv/get', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            key,
            allowExpired,
            withTTL,
            withExpireAt,
          }),
        });
        const data = await res.json();
        if (!data.success) throw new Error(data.error);
        setResponse(data.data);
      } else if (activeTab === 'delete') {
        const res = await fetch('/api/titankv/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ key }),
        });
        const data = await res.json();
        if (!data.success) throw new Error(data.error);
        setResponse(data.data);
      } else if (activeTab === 'ttl') {
        const payload: any = { key };
        if (ttlOpType === 'set') payload.ttl = Number(ttlDeltaVal);
        else if (ttlOpType === 'increase') payload.increase = Number(ttlDeltaVal);
        else if (ttlOpType === 'reduce') payload.reduce = Number(ttlDeltaVal);

        const res = await fetch('/api/titankv/ttl', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const data = await res.json();
        if (!data.success) throw new Error(data.error);
        setResponse(data.data);
      }

      setLatencyMs(Number((performance.now() - start).toFixed(2)));
      onSuccessAction?.();
    } catch (err: any) {
      setErrorMsg(err.message || 'Operation failed');
      setLatencyMs(Number((performance.now() - start).toFixed(2)));
    } finally {
      setLoading(false);
    }
  };

  const handlePreFill = (presetKey: string) => {
    setKey(presetKey);
    if (activeTab === 'put') {
      if (presetKey.startsWith('session:')) {
        setValJson(
          JSON.stringify(
            { sessionId: presetKey, authenticated: true, expires: Date.now() + 1800000, roles: ['user'] },
            null,
            2
          )
        );
        setTtlMs('1800000');
      } else if (presetKey.startsWith('cache:rate:')) {
        setValJson(JSON.stringify({ counter: 42, maxLimit: 1000, windowStart: Date.now() }, null, 2));
        setTtlMs('60000');
      }
    }
  };

  return (
    <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-2xl p-5 mb-8 shadow-sm">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-zinc-800 gap-3">
        <div>
          <h2 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
            <Database className="h-4 w-4 text-orange-400" />
            <span>Interactive KV Workbench</span>
          </h2>
          <p className="text-xs text-zinc-400">
            Execute native operations directly against the Rust engine with zero Node.js blocking
          </p>
        </div>

        {/* Tab selection */}
        <div className="flex bg-zinc-950 p-1 rounded-xl border border-zinc-800 self-start sm:self-auto">
          <button
            onClick={() => setActiveTab('put')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
              activeTab === 'put'
                ? 'bg-orange-500 text-white shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            PUT
          </button>
          <button
            onClick={() => setActiveTab('get')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
              activeTab === 'get'
                ? 'bg-orange-500 text-white shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            GET
          </button>
          <button
            onClick={() => setActiveTab('ttl')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
              activeTab === 'ttl'
                ? 'bg-orange-500 text-white shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            TTL
          </button>
          <button
            onClick={() => setActiveTab('delete')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
              activeTab === 'delete'
                ? 'bg-red-500 text-white shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            DELETE
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-5 mt-5">
        {/* Left: Input Form (7 cols) */}
        <div className="lg:col-span-7 space-y-4">
          {/* Key input */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-xs font-medium text-zinc-300">Target Key (max 64 bytes)</label>
              <div className="flex gap-1.5">
                <button
                  type="button"
                  onClick={() => handlePreFill('session:usr_98124')}
                  className="text-[10px] text-zinc-400 hover:text-orange-400 underline"
                >
                  session:usr
                </button>
                <button
                  type="button"
                  onClick={() => handlePreFill('config:rate_limiter')}
                  className="text-[10px] text-zinc-400 hover:text-orange-400 underline"
                >
                  config:rate
                </button>
                <button
                  type="button"
                  onClick={() => handlePreFill('product:cache:sku_4920')}
                  className="text-[10px] text-zinc-400 hover:text-orange-400 underline"
                >
                  product:sku
                </button>
              </div>
            </div>
            <input
              type="text"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="e.g. user:profile:1001"
              className="w-full px-3.5 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-sm font-mono text-zinc-100 focus:outline-none focus:border-orange-500 transition"
            />
          </div>

          {/* PUT Tab fields */}
          {activeTab === 'put' && (
            <>
              <div>
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-xs font-medium text-zinc-300">Value (Raw Buffer / JSON Cache)</label>
                  <button
                    type="button"
                    onClick={() => {
                      try {
                        setValJson(JSON.stringify(JSON.parse(valJson), null, 2));
                      } catch {}
                    }}
                    className="text-[10px] text-zinc-400 hover:text-orange-400"
                  >
                    Format JSON
                  </button>
                </div>
                <textarea
                  rows={6}
                  value={valJson}
                  onChange={(e) => setValJson(e.target.value)}
                  className="w-full px-3.5 py-2.5 rounded-xl bg-zinc-950 border border-zinc-800 text-xs font-mono text-zinc-200 focus:outline-none focus:border-orange-500 transition resize-y"
                />
              </div>

              {/* Mode & TTL Configuration */}
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div>
                  <label className="text-xs font-medium text-zinc-300 block mb-1">Write Mode</label>
                  <select
                    value={mode}
                    onChange={(e: any) => setMode(e.target.value)}
                    className="w-full px-3 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-xs text-zinc-200 focus:outline-none focus:border-orange-500 transition"
                  >
                    <option value="overwrite">overwrite (direct replace)</option>
                    <option value="skip">skip (keep existing valid key)</option>
                    <option value="ttl_compare">ttl_compare (check TTL diff threshold)</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-medium text-zinc-300 block mb-1">TTL (Milliseconds)</label>
                  <input
                    type="number"
                    value={ttlMs}
                    onChange={(e) => setTtlMs(e.target.value)}
                    placeholder="Leave empty for permanent"
                    className="w-full px-3 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-xs font-mono text-zinc-200 focus:outline-none focus:border-orange-500 transition"
                  />
                </div>
              </div>

              {mode === 'ttl_compare' && (
                <div>
                  <label className="text-xs font-medium text-zinc-300 block mb-1">TTL Diff Threshold (ms)</label>
                  <input
                    type="number"
                    value={ttlDiff}
                    onChange={(e) => setTtlDiff(e.target.value)}
                    className="w-full px-3 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-xs font-mono text-zinc-200 focus:outline-none focus:border-orange-500 transition"
                  />
                  <span className="text-[10px] text-zinc-500 mt-0.5 block">
                    Overwrite will only occur if (newExpireAt - oldExpireAt) &gt;= threshold
                  </span>
                </div>
              )}

              <div className="flex items-center gap-2 pt-1">
                <input
                  type="checkbox"
                  id="syncCheck"
                  checked={syncWrite}
                  onChange={(e) => setSyncWrite(e.target.checked)}
                  className="rounded border-zinc-700 text-orange-500 focus:ring-0"
                />
                <label htmlFor="syncCheck" className="text-xs text-zinc-300 cursor-pointer">
                  Synchronous disk fsync (durable write marker)
                </label>
              </div>
            </>
          )}

          {/* GET Tab fields */}
          {activeTab === 'get' && (
            <div className="p-4 rounded-xl bg-zinc-950/60 border border-zinc-800/80 space-y-3">
              <span className="text-xs font-medium text-zinc-300 block">Read Options</span>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                <label className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={withTTL}
                    onChange={(e) => setWithTTL(e.target.checked)}
                    className="rounded border-zinc-700 text-orange-500 focus:ring-0"
                  />
                  <span>withTTL (remaining ms)</span>
                </label>
                <label className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={withExpireAt}
                    onChange={(e) => setWithExpireAt(e.target.checked)}
                    className="rounded border-zinc-700 text-orange-500 focus:ring-0"
                  />
                  <span>withExpireAt (timestamp)</span>
                </label>
                <label className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={allowExpired}
                    onChange={(e) => setAllowExpired(e.target.checked)}
                    className="rounded border-zinc-700 text-orange-500 focus:ring-0"
                  />
                  <span>allowExpired</span>
                </label>
              </div>
            </div>
          )}

          {/* TTL Tab fields */}
          {activeTab === 'ttl' && (
            <div className="p-4 rounded-xl bg-zinc-950/60 border border-zinc-800/80 space-y-3">
              <span className="text-xs font-medium text-zinc-300 block">TTL Operation</span>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                {[
                  { id: 'query', label: 'Query TTL' },
                  { id: 'set', label: 'Set New TTL' },
                  { id: 'increase', label: 'Increase (+ms)' },
                  { id: 'reduce', label: 'Reduce (-ms)' },
                ].map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => setTtlOpType(item.id as any)}
                    className={`py-1.5 px-2 rounded-lg text-xs font-medium border text-center transition ${
                      ttlOpType === item.id
                        ? 'bg-orange-500/20 border-orange-500/50 text-orange-300'
                        : 'bg-zinc-900 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                    }`}
                  >
                    {item.label}
                  </button>
                ))}
              </div>

              {ttlOpType !== 'query' && (
                <div>
                  <label className="text-xs font-medium text-zinc-300 block mb-1">
                    {ttlOpType === 'set' && 'New TTL duration (ms)'}
                    {ttlOpType === 'increase' && 'Milliseconds to add'}
                    {ttlOpType === 'reduce' && 'Milliseconds to subtract'}
                  </label>
                  <input
                    type="number"
                    value={ttlDeltaVal}
                    onChange={(e) => setTtlDeltaVal(e.target.value)}
                    className="w-full px-3 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-xs font-mono text-zinc-200 focus:outline-none focus:border-orange-500"
                  />
                </div>
              )}
            </div>
          )}

          {/* DELETE Tab note */}
          {activeTab === 'delete' && (
            <div className="p-4 rounded-xl bg-red-950/20 border border-red-900/30 text-xs text-red-300">
              Executing delete appends a tombstone record with sequence marker into the Segment Value Log and clears the bucket slot directory.
            </div>
          )}

          <button
            type="button"
            onClick={handleExecute}
            disabled={loading}
            className={`w-full py-2.5 px-4 rounded-xl text-xs font-semibold flex items-center justify-center gap-2 transition shadow-md disabled:opacity-50 ${
              activeTab === 'delete'
                ? 'bg-red-600 hover:bg-red-500 text-white shadow-red-600/20'
                : 'bg-orange-600 hover:bg-orange-500 text-white shadow-orange-600/20'
            }`}
          >
            {loading ? (
              <span className="flex items-center gap-2">
                <span className="h-3.5 w-3.5 border-2 border-white/20 border-t-white rounded-full animate-spin"></span>
                Executing on Native Rust Engine...
              </span>
            ) : (
              <span className="flex items-center gap-2">
                <Play className="h-3.5 w-3.5 fill-current" />
                Execute {activeTab.toUpperCase()}
              </span>
            )}
          </button>
        </div>

        {/* Right: Output Console (5 cols) */}
        <div className="lg:col-span-5 flex flex-col">
          <div className="flex items-center justify-between pb-2 mb-2 border-b border-zinc-800">
            <span className="text-xs font-medium text-zinc-300">Engine Response & Telemetry</span>
            {latencyMs !== null && (
              <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                {latencyMs} ms
              </span>
            )}
          </div>

          <div className="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl p-3.5 font-mono text-xs overflow-auto min-h-[260px] max-h-[380px] flex flex-col justify-between">
            {errorMsg ? (
              <div className="text-red-400 flex items-start gap-2">
                <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
                <div>
                  <div className="font-bold">Error</div>
                  <div>{errorMsg}</div>
                </div>
              </div>
            ) : response !== null ? (
              <pre className="text-zinc-200 whitespace-pre-wrap break-all leading-relaxed">
                {typeof response === 'object' ? JSON.stringify(response, null, 2) : String(response)}
              </pre>
            ) : (
              <div className="text-zinc-500 italic flex flex-col items-center justify-center h-full text-center">
                <CornerDownLeft className="h-6 w-6 text-zinc-600 mb-2" />
                <span>Ready for execution. Click 'Execute' above.</span>
              </div>
            )}

            {response && (
              <div className="pt-3 mt-3 border-t border-zinc-800/80 flex items-center justify-between text-[11px] text-zinc-400">
                <span>Native Zero-Copy Buffer</span>
                <span className="text-orange-400 font-semibold">Status: 200 OK</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
