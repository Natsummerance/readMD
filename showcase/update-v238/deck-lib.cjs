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
  if (manifest.schema_version !== 1) throw new Error('Unsupported V2.3.8 deck manifest schema');
  if (manifest.release !== 'v2.3.8' || manifest.range !== 'v2.3.7..v2.3.8') {
    throw new Error('Deck manifest must describe the v2.3.7..v2.3.8 release range');
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
    if (!page.copy || typeof page.copy !== 'object') throw new Error(`${page.id} is missing copy`);
    for (const locale of LOCALES) {
      const copy = page.copy[locale];
      if (!copy) throw new Error(`${page.id} is missing ${locale} copy`);
      for (const field of ['eyebrow', 'title', 'body', 'note']) {
        if (!copy[field] || typeof copy[field] !== 'string') {
          throw new Error(`${page.id} ${locale} copy is missing ${field}`);
        }
      }
    }
  }
  return manifest;
}

function resolveSources(manifest, repoRoot, captureDir) {
  const shotLibrary = readJson(path.join(repoRoot, 'showcase', 'shot_library.json'));
  const shots = shotLibrary.shots || {};
  const externalSources = manifest.sources || {};
  return manifest.pages.map((page) => {
    let sourcePath;
    let sourceDescription;
    let sourceFeature;
    let kind;
    let sourceId;
    if (page.shot_id) {
      const shot = shots[page.shot_id];
      if (!shot) throw new Error(`Unknown shot id: ${page.shot_id}`);
      sourcePath = path.join(captureDir, shot.output);
      sourceDescription = shot.description;
      sourceFeature = shot.name;
      kind = 'captured-shot';
      sourceId = page.shot_id;
    } else {
      const source = externalSources[page.source_id];
      if (!source) throw new Error(`Unknown source id: ${page.source_id}`);
      sourcePath = path.join(repoRoot, source.path);
      sourceDescription = source.description;
      sourceFeature = source.description;
      kind = source.type;
      sourceId = page.source_id;
    }
    if (!fs.existsSync(sourcePath)) throw new Error(`Source screenshot does not exist: ${sourcePath}`);
    if (!isPng(sourcePath)) throw new Error(`Source screenshot is not a valid PNG: ${sourcePath}`);
    return {
      ...page,
      source: {
        id: sourceId,
        kind,
        path: sourcePath,
        sha256: sha256(sourcePath),
        description: sourceDescription,
        feature: sourceFeature,
      },
    };
  });
}

function updateListMarkdown(manifest, locale) {
  const isChinese = locale === 'zh-CN';
  const lines = [
    `# ReadMD ${manifest.release} ${isChinese ? '大版本更新清单' : 'Release update list'}`,
    '',
    isChinese
      ? `本清单约束 ${manifest.range} 的 14 张海报与发布证据。每一页均对应真实截图、功能来源与独立 SHA-256。`
      : `This list defines the 14 poster evidence chain for ${manifest.range}. Every page resolves to an authentic screenshot, source files and SHA-256 hash.`,
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
  const repoRoot = path.resolve(showcaseRoot, '..');
  const manifest = loadManifest(manifestPath);
  const pages = resolveSources(manifest, repoRoot, captureDir);
  return { manifest, pages };
}

module.exports = {
  isPng,
  loadManifest,
  requireInside,
  resolveSources,
  sha256,
  updateListMarkdown,
  validateDeck,
};
