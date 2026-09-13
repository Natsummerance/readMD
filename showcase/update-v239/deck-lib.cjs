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
  if (manifest.schema_version !== 1) throw new Error('Unsupported V2.3.9 deck manifest schema');
  if (manifest.release !== 'v2.3.9' || manifest.range !== 'v2.3.8..v2.3.9') {
    throw new Error('Deck manifest must describe the v2.3.8..v2.3.9 release range');
  }
  if (!Array.isArray(manifest.pages) || manifest.pages.length < 10 || manifest.pages.length > 20) {
    throw new Error('Deck must contain between 10 and 20 evidence pages');
  }
  const ids = new Set();
  const sourceRefs = new Set();
  for (const page of manifest.pages) {
    if (!/^[a-z0-9-]+$/.test(String(page.id || '')) || ids.has(page.id)) {
      throw new Error(`Invalid or duplicate deck page id: ${page.id}`);
    }
    ids.add(page.id);
    if (!LAYOUTS.has(page.layout)) throw new Error(`${page.id} uses an unknown layout: ${page.layout}`);
    if (!Array.isArray(page.evidence) || page.evidence.length === 0) throw new Error(`${page.id} has no source evidence`);
    const sourceRef = page.shot_id ? `shot:${page.shot_id}` : `source:${page.source_id}`;
    if (!page.shot_id && !page.source_id) throw new Error(`${page.id} needs a screenshot source`);
    if (sourceRefs.has(sourceRef)) throw new Error(`A screenshot source is reused by ${page.id}: ${sourceRef}`);
    sourceRefs.add(sourceRef);
    for (const locale of LOCALES) {
      const copy = page.copy?.[locale];
      if (!copy) throw new Error(`${page.id} is missing copy for ${locale}`);
      if (!copy.title || !copy.body || !copy.note || !copy.eyebrow) {
        throw new Error(`${page.id} copy incomplete for ${locale}`);
      }
    }
  }
  return manifest;
}

function updateListMarkdown(manifest, locale) {
  const isChinese = locale === 'zh-CN';
  const lines = [
    `# ReadMD ${manifest.release} ${isChinese ? '大版本更新清单' : 'Release update list'}`,
    '',
    isChinese
      ? `本清单约束 ${manifest.range} 的 ${manifest.pages.length} 张海报与发布证据。每一页均对应真实截图、功能来源与独立 SHA-256。`
      : `This list defines the ${manifest.pages.length} poster evidence chain for ${manifest.range}. Every page resolves to an authentic screenshot, source files and SHA-256 hash.`,
    '',
    `| ${isChinese ? '序号' : 'No.'} | ${isChinese ? '模块' : 'Area'} | ${isChinese ? '功能' : 'Feature'} | ${isChinese ? '说明' : 'Summary'} | ${isChinese ? '代码证据' : 'Code evidence'} |`,
    '|---|---|---|---|---|',
  ];
  manifest.pages.forEach((page, index) => {
    const copy = page.copy[locale];
    const evidence = page.evidence.map((item) => `\`${item}\``).join('<br>');
    lines.push(`| ${String(index + 1).padStart(2, '0')} | ${copy.eyebrow} | **${copy.title}** | ${copy.body} | ${evidence} |`);
  });
  lines.push('');
  return lines.join('\n');
}

function validateDeck({ manifestPath, showcaseRoot, captureDir }) {
  const manifest = loadManifest(manifestPath);
  const repoRoot = path.resolve(showcaseRoot, '..');
  const pages = [];
  const sources = new Map();

  for (const page of manifest.pages) {
    let sourcePath;
    let sourceId;
    let kind;

    if (page.shot_id) {
      sourceId = page.shot_id;
      kind = 'captured-shot';
      const fileCandidate = path.join(captureDir, `${page.shot_id}.png`);
      sourcePath = fileCandidate;
    } else {
      sourceId = page.source_id;
      kind = 'manifest-source';
      const declaration = manifest.sources?.[page.source_id];
      if (!declaration) throw new Error(`Unknown source_id: ${page.source_id}`);
      sourcePath = path.resolve(repoRoot, declaration.path);
    }

    if (!fs.existsSync(sourcePath)) {
      throw new Error(`Screenshot file missing for ${page.id}: ${sourcePath}`);
    }
    if (!isPng(sourcePath)) {
      throw new Error(`Screenshot must be valid PNG for ${page.id}: ${sourcePath}`);
    }

    const digest = sha256(sourcePath);
    if (sources.has(digest)) {
      throw new Error(`Duplicate image content detected between ${sources.get(digest)} and ${page.id}`);
    }
    sources.set(digest, page.id);

    pages.push({
      ...page,
      source: { id: sourceId, kind, path: sourcePath, sha256: digest }
    });
  }

  return { manifest, pages };
}

module.exports = {
  LAYOUTS,
  LOCALES,
  isPng,
  loadManifest,
  readJson,
  requireInside,
  sha256,
  updateListMarkdown,
  validateDeck
};
