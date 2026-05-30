// Tiny static server for the SEELE compendium (Node built-ins only, no deps, no network fetch).
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, sep } from 'node:path';

const ROOT = normalize('C:/dev/tools/SEELE/docs/compendium');
const PORT = 8137;
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.md':   'text/plain; charset=utf-8',   // text/plain so the browser shows it inline, not a download
  '.css':  'text/css; charset=utf-8',
  '.js':   'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
};

const server = createServer(async (req, res) => {
  try {
    let p = decodeURIComponent((req.url || '/').split('?')[0]);
    if (p === '/' || p === '') p = '/SEELE-COMPENDIUM.html';
    const full = normalize(join(ROOT, p));
    if (full !== ROOT && !full.startsWith(ROOT + sep)) {   // block path traversal
      res.writeHead(403); return res.end('forbidden');
    }
    const data = await readFile(full);
    res.writeHead(200, { 'content-type': MIME[extname(full).toLowerCase()] || 'application/octet-stream' });
    res.end(data);
  } catch {
    res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    res.end('not found');
  }
});

server.on('error', (e) => { console.error('server error:', e.message); process.exit(1); });
server.listen(PORT, '127.0.0.1', () => {
  console.log(`SEELE compendium served at http://localhost:${PORT}/`);
  console.log(`  http://localhost:${PORT}/SEELE-COMPENDIUM.html`);
  console.log(`  http://localhost:${PORT}/SEELE-COMPENDIUM.md`);
});
