import { Router } from 'express';
import { titankvService } from './titankvService.ts';

export const titankvRouter = Router();

titankvRouter.get('/stats', async (_req, res) => {
  try {
    const stats = await titankvService.stats();
    res.json({ success: true, data: stats });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/put', async (req, res) => {
  try {
    const { key, value, mode, ttl, ttlDiff, sync } = req.body;
    if (!key) {
      return res.status(400).json({ success: false, error: 'Key is required' });
    }
    const result = await titankvService.put(key, value, {
      mode,
      ttl: ttl ? Number(ttl) : undefined,
      ttlDiff: ttlDiff ? Number(ttlDiff) : undefined,
      sync: Boolean(sync),
    });
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/get', async (req, res) => {
  try {
    const { key, allowExpired, withTTL, withExpireAt } = req.body;
    if (!key) {
      return res.status(400).json({ success: false, error: 'Key is required' });
    }
    const result = await titankvService.get(key, {
      allowExpired: allowExpired !== undefined ? Boolean(allowExpired) : true,
      withTTL: Boolean(withTTL),
      withExpireAt: Boolean(withExpireAt),
    });
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/delete', async (req, res) => {
  try {
    const { key } = req.body;
    if (!key) {
      return res.status(400).json({ success: false, error: 'Key is required' });
    }
    const result = await titankvService.delete(key);
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/ttl', async (req, res) => {
  try {
    const { key, ttl, increase, reduce } = req.body;
    if (!key) {
      return res.status(400).json({ success: false, error: 'Key is required' });
    }
    const options: any = {};
    if (ttl !== undefined) options.ttl = Number(ttl);
    if (increase !== undefined) options.increase = Number(increase);
    if (reduce !== undefined) options.reduce = Number(reduce);

    const result = await titankvService.ttl(key, options);
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/benchmark', async (req, res) => {
  try {
    const { totalKeys, batchSize, concurrency, payloadSize, ttlMs } = req.body;
    const result = await titankvService.runBenchmark({
      totalKeys: Number(totalKeys || 1000),
      batchSize: Number(batchSize || 50),
      concurrency: Number(concurrency || 4),
      payloadSize: Number(payloadSize || 256),
      ttlMs: ttlMs !== undefined ? Number(ttlMs) : 300000,
    });
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/reset', async (_req, res) => {
  try {
    const result = await titankvService.resetData();
    res.json({ success: true, data: result });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});

titankvRouter.post('/seed-sample', async (_req, res) => {
  try {
    const sampleKeys = [
      {
        key: 'session:usr_98124',
        value: { userId: 98124, username: 'alex_dev', roles: ['admin', 'maintainer'], ip: '192.168.1.104', activeSession: true },
        ttl: 1800000, // 30 mins
      },
      {
        key: 'config:rate_limiter',
        value: { maxQps: 100000, burst: 250000, windowMs: 1000, enabled: true },
      },
      {
        key: 'product:cache:sku_4920',
        value: {
          id: 'sku_4920',
          title: 'Titan Storage Unit 1TB NVMe Gen4',
          price: 149.99,
          inventory: 842,
          specs: { r_seq: '7450 MB/s', w_seq: '6900 MB/s', iops: 1000000 },
        },
        ttl: 600000, // 10 mins
      },
      {
        key: 'device:telemetry:node_1',
        value: { cpuPercent: 14.2, memUsedMb: 512, ioWriteBps: 45000000, activeLanes: 64 },
        ttl: 60000, // 1 min
      },
    ];

    for (const item of sampleKeys) {
      await titankvService.put(item.key, item.value, { ttl: item.ttl });
    }

    const stats = await titankvService.stats();
    res.json({ success: true, seeded: sampleKeys.length, stats });
  } catch (err: any) {
    res.status(500).json({ success: false, error: err.message });
  }
});
