import express from 'express';
import path from 'path';
import { titankvRouter } from './src/server/router.ts';

const app = express();
const port = process.env.PORT || 3000;

app.use(express.json({ limit: '50mb' }));
app.use('/api/titankv', titankvRouter);

const distPath = path.resolve(process.cwd(), 'dist');
app.use(express.static(distPath));

app.get('*', (_req, res) => {
  res.sendFile(path.resolve(distPath, 'index.html'));
});

app.listen(port, () => {
  console.log(`TitanKV Storage Engine running on port ${port}`);
});
