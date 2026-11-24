import 'dotenv/config';
import express from 'express';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer } from 'ws';
import { serveSocket } from './server/session.ts';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function normalizeGatewayToken(raw: string | undefined): string {
  const trimmed = (raw ?? '').trim().replace(/^["']|["']$/g, '').trim();
  if (!trimmed || /^(MY_GATEWAY_TOKEN|YOUR_GATEWAY_TOKEN|CHANGE_ME|PLACEHOLDER)$/i.test(trimmed)) {
    return '';
  }
  return trimmed;
}

async function startServer() {
  const app = express();
  const port = Number(process.env.PORT) || 3000;
  const gatewayToken = normalizeGatewayToken(process.env.RUSTDESK_GATEWAY_TOKEN);

  app.disable('x-powered-by');

  app.get('/api/health', (_req, res) => {
    res.json({
      ok: true,
      service: 'rustdesk-web-gateway',
      authRequired: gatewayToken.length > 0,
    });
  });

  const httpServer = http.createServer(app);
  const wss = new WebSocketServer({ noServer: true, maxPayload: 4 * 1024 * 1024 });

  wss.on('error', (err) => {
    console.error('WebSocketServer error:', err);
  });

  httpServer.on('clientError', (_err, socket) => {
    if (socket.writable) {
      socket.end('HTTP/1.1 400 Bad Request\r\n\r\n');
    } else {
      socket.destroy();
    }
  });

  httpServer.on('upgrade', (request, socket, head) => {
    let pathname = '';
    try {
      const reqUrl = new URL(request.url ?? '/', `http://${request.headers.host ?? 'localhost'}`);
      pathname = reqUrl.pathname;
    } catch {
      socket.destroy();
      return;
    }

    if (pathname === '/ws') {
      wss.handleUpgrade(request, socket, head, (ws) => {
        serveSocket(ws, { token: gatewayToken });
      });
    } else {
      socket.destroy();
    }
  });

  if (process.env.NODE_ENV !== 'production') {
    const { createServer: createViteServer } = await import('vite');
    const vite = await createViteServer({
      server: { middlewareMode: true, hmr: false },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    const distDir = path.resolve(__dirname, 'dist');
    app.use(express.static(distDir));
    app.get('*', (_req, res) => {
      res.sendFile(path.join(distDir, 'index.html'));
    });
  }

  httpServer.listen(port, '0.0.0.0', () => {
    console.log(`RustDesk Web Gateway listening on http://0.0.0.0:${port}`);
  });
}

startServer().catch((err) => {
  console.error('Failed to start server:', err);
  process.exit(1);
});
