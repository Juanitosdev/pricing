// Prepares webdist/ — the front-end that Tauri bundles into the desktop app.
//
// The app ships WITHOUT any pricing data: the boss loads their own CSVs at
// runtime (Upload screen), so we copy only index.html and write a data-free
// stub catalogue.js. That keeps the venues' prices out of both the repo and the
// distributed .exe, which in turn lets the update repo stay public with no token.
//
// (Local browser dev is unaffected: opening the real index.html at the repo root
// still auto-loads the local catalogue.js sitting next to it.)
import { mkdirSync, copyFileSync, existsSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'webdist');

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const index = join(root, 'index.html');
if (!existsSync(index)) throw new Error('index.html not found at repo root');
copyFileSync(index, join(out, 'index.html'));

// Data-free stub so <script src="catalogue.js"> resolves (no 404) and the app
// boots straight to the Upload screen.
writeFileSync(
  join(out, 'catalogue.js'),
  '// Shipped empty on purpose — load your venue CSVs from the Upload screen.\n'
);

console.log('web assets → webdist/ (index.html + empty catalogue.js — no data bundled)');
