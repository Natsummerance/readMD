import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import crypto from 'node:crypto';
import { API_URL, publishedRelease, updateReleaseHtml } from '../public/assets/release-contract.mjs';
const site = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const snapshot = path.join(site, 'published-release.json');
async function writeChanged(file, text) {
  try { if (await fs.readFile(file, 'utf8') === text) return; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const temporary = file + '.readmd-build-' + process.pid + '.tmp';
  await fs.writeFile(temporary, text);
  await fs.rename(temporary, file);
}
export async function refreshRelease() {
  const headers = { Accept: 'application/vnd.github+json' };
  if (process.env.GITHUB_TOKEN) headers.Authorization = 'Bearer ' + process.env.GITHUB_TOKEN;
  const response = await fetch(API_URL, { headers, signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw Error('Published release lookup failed: HTTP ' + response.status);
  const r = publishedRelease(await response.json());
  await fs.writeFile(snapshot, JSON.stringify(r, null, 2) + '\n');
  const file = path.join(site, 'release.json');
  const previous = JSON.parse(await fs.readFile(file, 'utf8'));
  await fs.writeFile(file, JSON.stringify({ ...previous, stable: r.version,
    published: r.updatedAt.slice(0, 10), repository: r.repository }, null, 2) + '\n');
  console.log('[published-release] Verified ' + r.releaseTag + ': ' + r.assets.length + ' published packages');
}
export async function syncRelease() {
  const r = publishedRelease(JSON.parse(await fs.readFile(snapshot, 'utf8')));
  const config = JSON.parse(await fs.readFile(path.join(site, 'release.json'), 'utf8'));
  if (config.stable !== r.version) throw Error('Published snapshot differs from website manifest');
  async function walk(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      if (entry.name === 'showcase') continue;
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) await walk(file);
      else if (entry.name.endsWith('.html')) await writeChanged(file, updateReleaseHtml(await fs.readFile(file, 'utf8'), r, {
        download: /[\\/]download[\\/]index\.html$/.test(file), historical: /[\\/]release-notes[\\/]/.test(file),
        candidate: /[\\/](?:ai-autocomplete|integrations)[\\/]/.test(file),
      }));
    }
  }
  for (const folder of ['public', 'dist']) {
    const directory = path.join(site, folder);
    try { await fs.access(directory); } catch { continue; }
    await walk(directory);
    await writeChanged(path.join(directory, 'version.json'), JSON.stringify(r, null, 2) + '\n');
    const headerFile = path.join(directory, '_headers');
    let headers = await fs.readFile(headerFile, 'utf8');
    for (const language of ['', 'zh-cn/', 'zh-tw/', 'ja/']) {
      const html = await fs.readFile(path.join(directory, language, 'index.html'), 'utf8');
      const block = /<script type="application\/ld\+json">([\s\S]*?)<\/script>/.exec(html)?.[1];
      if (block) {
        const hash = "'sha256-" + crypto.createHash('sha256').update(block).digest('base64') + "'";
        if (!headers.includes(hash)) headers = headers.replace("script-src 'self'", "script-src 'self' " + hash);
      }
    }
    await writeChanged(headerFile, headers);
  }
  console.log('[sync-version] Offline ' + r.releaseTag + ': pages, metadata and downloads synchronized');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.includes('--refresh')) await refreshRelease(); else await syncRelease();
}
