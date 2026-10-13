import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseDashboard } from './src/api/client';
export default defineConfig({ plugins: [react(), {
  name: 'local-report-preview',
  configureServer(server) {
    server.middlewares.use('/api/local-report', async (request, response) => {
      if (request.method !== 'GET') { response.statusCode = 405; response.end(); return; }
      try {
        const text = await readFile(resolve(import.meta.dirname, '../../.local-data/preview-dashboard.json'), 'utf8');
        const data = parseDashboard(JSON.parse(text));
        if (data.dataMode !== 'observed') throw new Error('Invalid local preview');
        response.setHeader('Content-Type', 'application/json'); response.setHeader('Cache-Control', 'no-store'); response.end(text);
      } catch { response.statusCode = 404; response.end('{"error":"Local report unavailable"}'); }
    });
  }
}], clearScreen: false, build: { sourcemap: false } });
