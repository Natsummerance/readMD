import { readFile, writeFile, mkdir } from 'node:fs/promises';
const release = JSON.parse(await readFile(new URL('../release.json', import.meta.url), 'utf8'));
if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(release.stable) || release.repository !== 'Natsummerance/rust-ReadMD') throw Error('Invalid published release manifest');
const releaseTag = 'V' + release.stable;
const assetsBaseUrl = 'https://github.com/' + release.repository + '/releases/download/' + releaseTag + '/';
const output = new URL('../dist/', import.meta.url);
await mkdir(output, { recursive: true });
await writeFile(new URL('version.json', output), JSON.stringify({ version: release.stable, releaseTag, candidate: release.candidate, updatedAt: release.published, assetsBaseUrl, checksumUrl: assetsBaseUrl + 'SHA256SUMS.txt' }, null, 2) + '\n');
console.log('[sync-version] Public downloads pinned to published ' + releaseTag);
