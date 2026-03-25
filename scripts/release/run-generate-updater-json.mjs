import { spawnSync } from 'node:child_process';
import path from 'node:path';

const repo = process.env.MIDWAY_GITHUB_REPOSITORY || process.env.GITHUB_REPOSITORY;
const channel = process.env.MIDWAY_RELEASE_CHANNEL || 'stable';
const releaseJson = process.env.MIDWAY_RELEASE_JSON || 'out/release.json';
const assetsDir = process.env.MIDWAY_RELEASE_ASSETS_DIR || 'release-artifacts';
const output = process.env.MIDWAY_UPDATER_OUTPUT || (channel === 'beta' ? 'out/latest-beta.json' : 'out/latest.json');

if (!repo) {
  console.error('Falta MIDWAY_GITHUB_REPOSITORY o GITHUB_REPOSITORY para generar latest.json.');
  process.exit(1);
}

const scriptPath = path.resolve('scripts/release/generate-updater-json.mjs');
const result = spawnSync(process.execPath, [
  scriptPath,
  '--release-json',
  releaseJson,
  '--assets-dir',
  assetsDir,
  '--repo',
  repo,
  '--channel',
  channel,
  '--output',
  output
], {
  stdio: 'inherit',
  env: process.env
});

process.exit(result.status ?? 1);
