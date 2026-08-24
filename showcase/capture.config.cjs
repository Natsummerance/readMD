'use strict';

const fs = require('fs');
const path = require('path');

function parseViewport(value) {
  const match = /^(\d+)x(\d+)$/.exec(String(value || ''));
  if (!match) throw new Error(`SHOWCASE_VIEWPORT must be WIDTHxHEIGHT, got: ${value}`);
  return { width: Number(match[1]), height: Number(match[2]) };
}

function loadCaptureConfig(env = process.env) {
  // A portrait desktop window preserves the complete workbench in the card-2
  // hero without cropping the authentic viewport screenshot.
  const viewport = parseViewport(env.SHOWCASE_VIEWPORT || '960x1280');
  const scale = Number(env.SHOWCASE_SCALE || '2');
  if (!Number.isInteger(scale) || scale < 1 || scale > 4) throw new Error('SHOWCASE_SCALE must be an integer from 1 to 4');
  return {
    release: env.SHOWCASE_RELEASE || 'v2.3.7-beta.3',
    locale: env.SHOWCASE_LOCALE || 'zh-CN',
    theme: env.SHOWCASE_THEME || 'dark',
    viewport,
    scale,
    outputDir: env.SHOWCASE_OUTPUT_DIR || 'raw',
  };
}

function loadShotLibrary(libraryPath = path.join(__dirname, 'shot_library.json')) {
  const library = JSON.parse(fs.readFileSync(libraryPath, 'utf8'));
  if (library.schema_version !== 1) throw new Error('Unsupported shot library schema');
  const ids = Object.keys(library.shots);
  if (ids.length !== new Set(ids).size) throw new Error('Duplicate shot id');
  for (const [id, shot] of Object.entries(library.shots)) {
    if (shot.id !== id) throw new Error(`Shot id mismatch: ${id}`);
    if (!Array.isArray(shot.assertions) || shot.assertions.length === 0) throw new Error(`${id} has no assertions`);
    if (!Array.isArray(shot.evidence) || shot.evidence.length === 0) throw new Error(`${id} has no evidence`);
    if (!/^[\w.-]+\.png$/.test(shot.output)) throw new Error(`${id} has unsafe output name`);
  }
  return library;
}

module.exports = { loadCaptureConfig, loadShotLibrary, parseViewport };
