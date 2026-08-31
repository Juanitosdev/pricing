// Fill the updater placeholders in src-tauri/tauri.conf.json.
//
// Usage:
//   node scripts/configure-updater.mjs <owner/repo> [<pubkey | path-to-.pub>]
//
// Examples:
//   node scripts/configure-updater.mjs tbg/pricing
//   node scripts/configure-updater.mjs tbg/pricing tbg-pricing-updater.key.pub
//
// The public key is the SECOND file printed by `npm run signer:generate`
// (the one ending in .pub). Its private counterpart must NEVER be committed —
// it goes into the GitHub secret TAURI_SIGNING_PRIVATE_KEY instead.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const confPath = join(root, 'src-tauri', 'tauri.conf.json');

const [slug, pub] = process.argv.slice(2);
if (!slug || !/^[^/]+\/[^/]+$/.test(slug)) {
  console.error('Usage: node scripts/configure-updater.mjs <owner/repo> [<pubkey | path-to-.pub>]');
  process.exit(1);
}
const [owner, repo] = slug.split('/');

let conf = readFileSync(confPath, 'utf8');
conf = conf.replaceAll('__GH_OWNER__', owner).replaceAll('__GH_REPO__', repo);

if (pub) {
  const key = existsSync(pub) ? readFileSync(pub, 'utf8').trim() : pub.trim();
  conf = conf.replace('__UPDATER_PUBKEY__', key);
}

writeFileSync(confPath, conf);
console.log(`updater endpoint → github.com/${owner}/${repo}`);
console.log(pub ? 'public key written into tauri.conf.json' : 'note: pubkey not set yet (pass it as the 2nd argument)');
