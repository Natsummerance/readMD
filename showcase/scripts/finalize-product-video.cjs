'use strict';

const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const input = path.resolve(root, process.env.PRODUCT_FILM_INPUT || 'raw/product-journey.webm');
const output = path.resolve(root, process.env.PRODUCT_FILM_OUTPUT || '../website/public/media/readmd-product-journey.mp4');

if (!fs.existsSync(input)) throw new Error(`Recording not found: ${input}`);

function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', shell: false });
  if (result.status !== 0) {
    throw new Error(`${command} failed:\n${result.stdout}\n${result.stderr}`);
  }
  return result;
}

const temporary = `${output}.tmp.mp4`;
const temporaryWebm = `${output}.tmp.webm`;
run('ffmpeg', [
  '-y',
  '-i', input,
  '-ss', '0.7',
  '-vf', 'scale=1440:810:force_original_aspect_ratio=increase,crop=1440:810,fps=30',
  '-c:v', 'libx264',
  '-profile:v', 'high',
  '-preset', 'veryfast',
  '-crf', '27',
  '-pix_fmt', 'yuv420p',
  '-movflags', '+faststart',
  '-an',
  temporary,
]);

run('ffmpeg', [
  '-y',
  '-i', input,
  '-ss', '0.7',
  '-vf', 'scale=1440:810:force_original_aspect_ratio=increase,crop=1440:810,fps=30',
  '-c:v', 'libvpx-vp9',
  '-b:v', '0',
  '-crf', '33',
  '-row-mt', '1',
  '-cpu-used', '4',
  '-pix_fmt', 'yuv420p',
  '-force_key_frames', 'expr:gte(t,n_forced*1)',
  '-an',
  temporaryWebm,
]);

const probe = spawnSync('ffprobe', [
  '-v', 'error',
  '-select_streams', 'v:0',
  '-show_entries', 'format=duration,size:stream=codec_name,width,height',
  '-of', 'json',
  temporary,
], { encoding: 'utf8', shell: false });
if (probe.status !== 0) throw new Error(probe.stderr);
const media = JSON.parse(probe.stdout);
const duration = Number(media.format.duration);
if (!(duration >= 5)) throw new Error(`Recorded film too short: ${duration}s`);

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.renameSync(temporary, output);
fs.renameSync(temporaryWebm, path.join(path.dirname(output), 'readmd-product-journey.webm'));

const evidencePath = path.resolve(root, 'reports/product_film_evidence.json');
fs.writeFileSync(evidencePath, JSON.stringify({
  schema_version: 1,
  source_provider: 'playwright-native-video',
  composition_reference: {
    repository: 'webadderallorg/Recordly',
    stars: 22497,
    license: 'AGPL-3.0',
    usage: 'patterns only; no Recordly code imported',
  },
  automation_provider: { repository: 'microsoft/playwright', integration: 'browser context recordVideo' },
  encoder: 'ffmpeg/libx264',
  source: path.relative(root, input),
  output: path.relative(root, output),
  duration_seconds: duration,
  bytes: Number(media.format.size),
  stream: media.streams[0],
  captured_at: new Date().toISOString(),
}, null, 2), 'utf8');

console.log(JSON.stringify({ ok: true, output, duration_seconds: duration, bytes: Number(media.format.size) }, null, 2));
