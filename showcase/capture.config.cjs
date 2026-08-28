'use strict';

const fs = require('fs');
const path = require('path');

function parseViewport(value) {
  const match = /^(\d+)x(\d+)$/.exec(String(value || ''));
  if (!match) throw new Error(`SHOWCASE_VIEWPORT must be WIDTHxHEIGHT, got: ${value}`);
  return { width: Number(match[1]), height: Number(match[2]) };
}

function loadCaptureConfig(env = process.env) {
  // ReadMD is a landscape desktop application; capture the full workbench at a
  // 16:9 stage so website media does not require portrait cropping.
  const viewport = parseViewport(env.SHOWCASE_VIEWPORT || '1440x810');
  const scale = Number(env.SHOWCASE_SCALE || '2');
  if (!Number.isInteger(scale) || scale < 1 || scale > 4) throw new Error('SHOWCASE_SCALE must be an integer from 1 to 4');
  return {
    release: env.SHOWCASE_RELEASE || 'v2.3.7',
    locale: env.SHOWCASE_LOCALE || 'zh-CN',
    theme: env.SHOWCASE_THEME || 'dark',
    viewport,
    scale,
    outputDir: env.SHOWCASE_OUTPUT_DIR || 'raw',
  };
}

function loadShotLibrary(libraryPath = path.join(__dirname, 'shot_library.json'), overlayPath = process.env.SHOWCASE_SHOT_OVERLAY) {
  const library = JSON.parse(fs.readFileSync(libraryPath, 'utf8'));
  if (library.schema_version !== 1) throw new Error('Unsupported shot library schema');
  const ids = Object.keys(library.shots);
  if (ids.length !== new Set(ids).size) throw new Error('Duplicate shot id');
  for (const [id, shot] of Object.entries(library.shots)) {
    if (shot.id !== id) throw new Error(`Shot id mismatch: ${id}`);
    if (!Array.isArray(shot.assertions) || shot.assertions.length === 0) throw new Error(`${id} has no assertions`);
    if (!Array.isArray(shot.evidence) || shot.evidence.length === 0) throw new Error(`${id} has no evidence`);
    if (shot.fixture !== undefined && !/^[\w.-]+\.md$/.test(shot.fixture)) throw new Error(`${id} has unsafe fixture`);
    if (shot.viewport !== undefined) {
      if (!Number.isInteger(shot.viewport.width) || shot.viewport.width < 640 || shot.viewport.width > 2560) throw new Error(`${id} has invalid viewport width`);
      if (!Number.isInteger(shot.viewport.height) || shot.viewport.height < 480 || shot.viewport.height > 2560) throw new Error(`${id} has invalid viewport height`);
    }
    if (!/^[\w.-]+\.png$/.test(shot.output)) throw new Error(`${id} has unsafe output name`);
  }
  if (!overlayPath) return library;
  const overlay = JSON.parse(fs.readFileSync(overlayPath, 'utf8'));
  if (overlay.schema_version !== 1) throw new Error('Shot overlay schema_version must be 1');
  for (const [id, patch] of Object.entries(overlay.shots || {})) {
    const shot = library.shots[id];
    if (!shot) throw new Error(`Shot overlay references unknown shot: ${id}`);
    library.shots[id] = { ...shot, ...patch, id, output: shot.output };
  }
  return library;
}

module.exports = { loadCaptureConfig, loadShotLibrary, parseViewport };
