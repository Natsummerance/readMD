'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const LOCALES = ['zh-CN', 'en'];
const LAYOUTS = new Set(['specimen', 'ledger', 'blueprint', 'archive']);

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function sha256(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function requireInside(root, target, label) {
  const rootPath = path.resolve(root);
  const targetPath = path.resolve(target);
  if (targetPath !== rootPath && !targetPath.startsWith(rootPath + path.sep)) {
    throw new Error(`${label} must stay inside ${rootPath}`);
  }
  return targetPath;
}

function isPng(filePath) {
  const handle = fs.openSync(filePath, 'r');
  try {
    const signature = Buffer.alloc(PNG_SIGNATURE.length);
    fs.readSync(handle, signature, 0, signature.length, 0);
    return signature.equals(PNG_SIGNATURE);
  } finally {
    fs.closeSync(handle);
  }
}

function loadManifest(manifestPath) {
  const manifest = readJson(manifestPath);
  if (manifest.schema_version !== 1) throw new Error('Unsupported V2.3.7 deck manifest schema');
  if (manifest.release !== 'v2.3.7' || manifest.range !== 'v2.3.6..v2.3.7') {
    throw new Error('Deck manifest must describe the v2.3.6..v2.3.7 release range');
  }
  if (!Array.isArray(manifest.pages) || manifest.pages.length < 10 || manifest.pages.length > 16) {
    throw new Error('Deck must contain between 10 and 16 evidence pages');
  }
  const ids = new Set();
  const sourceRefs = new Set();
  for (const page of manifest.pages) {
    if (!/^[a-z0-9-]+$/.test(String(page.id || '')) || ids.has(page.id)) {
      throw new Error(`Invalid or duplicate deck page id: ${page.id}`);
    }
    ids.add(page.id);
    if (!LAYOUTS.has(page.layout)) throw new Error(`${page.id} uses an unknown layout`);
    if (!Array.isArray(page.evidence) || page.evidence.length === 0) throw new Error(`${page.id} has no source evidence`);
    const sourceRef = page.shot_id ? `shot:${page.shot_id}` : `source:${page.source_id}`;
    if (!page.shot_id && !page.source_id) throw new Error(`${page.id} needs a screenshot source`);
    if (sourceRefs.has(sourceRef)) throw new Error(`A screenshot source is reused by ${page.id}: ${sourceRef}`);
    sourceRefs.add(sourceRef);
    for (const locale of LOCALES) {
      const copy = page.copy && page.copy[locale];
      for (const field of ['eyebrow', 'title', 'body', 'note']) {
        if (!String(copy && copy[field] || '').trim()) throw new Error(`${page.id} lacks ${locale} ${field}`);
      }
    }
  }
  for (const locale of LOCALES) {
    if (!String(manifest.title && manifest.title[locale] || '').trim()) throw new Error(`Deck lacks a ${locale} title`);
  }
  return manifest;
}

function resolveEvidence(manifest, showcaseRoot, captureDir) {
  const capturePath = requireInside(showcaseRoot, path.join(captureDir, 'capture.json'), 'Capture manifest');
  if (!fs.existsSync(capturePath)) throw new Error(`Missing current capture manifest: ${capturePath}`);
  const capture = readJson(capturePath);
  if (capture.schema_version !== 1 || capture.release !== manifest.release) {
    throw new Error(`Capture release must be ${manifest.release}`);
  }
  const shots = new Map((capture.shots || []).map((shot) => [shot.shot_id, shot]));
  const usedHashes = new Map();
  return manifest.pages.map((page) => {
    let filePath;
    let sourceSha;
    let sourceMeta;
    if (page.shot_id) {
      const shot = shots.get(page.shot_id);
      if (!shot || shot.release !== manifest.release || shot.authentic !== true) {
        throw new Error(`${page.id} has no authenticated ${manifest.release} capture for ${page.shot_id}`);
      }
      if (!/^raw\/[\w.-]+\.png$/.test(String(shot.file || ''))) throw new Error(`${page.id} has an unsafe capture path`);
      filePath = requireInside(showcaseRoot, path.join(showcaseRoot, shot.file), `${page.id} capture`);
      sourceSha = String(shot.sha256 || '');
      sourceMeta = { kind: 'captured-shot', id: page.shot_id, feature: shot.feature, capture: shot.capture };
    } else {
      const source = manifest.sources && manifest.sources[page.source_id];
      if (!source || source.type !== 'repository-file' || !/^showcase\/[\w.-]+(?:\/[\w.-]+)*\.png$/.test(String(source.path || ''))) {
        throw new Error(`${page.id} has an unsafe repository screenshot source`);
      }
      filePath = requireInside(showcaseRoot, path.resolve(showcaseRoot, '..', source.path), `${page.id} repository evidence`);
      sourceSha = sha256(filePath);
      sourceMeta = { kind: 'audited-repository-shot', id: page.source_id, description: source.description };
    }
    if (!fs.existsSync(filePath) || !isPng(filePath)) throw new Error(`${page.id} evidence is not a PNG: ${filePath}`);
    const actualSha = sha256(filePath);
    if (actualSha !== sourceSha) throw new Error(`${page.id} screenshot SHA-256 mismatch`);
    if (usedHashes.has(actualSha)) throw new Error(`${page.id} duplicates screenshot evidence from ${usedHashes.get(actualSha)}`);
    usedHashes.set(actualSha, page.id);
    return { ...page, source: { path: filePath, sha256: actualSha, ...sourceMeta } };
  });
}

function validateDeck({ manifestPath, showcaseRoot, captureDir }) {
  const manifest = loadManifest(manifestPath);
  const pages = resolveEvidence(manifest, showcaseRoot, captureDir);
  return { manifest, pages };
}

function updateListMarkdown(manifest, locale) {
  const title = manifest.title[locale];
  const rangeLine = locale === 'zh-CN' ? `版本范围：\`${manifest.range}\`` : `Release range: \`${manifest.range}\``;
  const rows = manifest.pages.map((page, index) => {
    const copy = page.copy[locale];
    return `${index + 1}. **${copy.title}** — ${copy.body}`;
  });
  return `# ${title}\n\n${rangeLine}\n\n${rows.join('\n\n')}\n`;
}

module.exports = {
  LAYOUTS,
  LOCALES,
  loadManifest,
  requireInside,
  sha256,
  updateListMarkdown,
  validateDeck,
};
