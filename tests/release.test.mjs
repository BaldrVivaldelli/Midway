import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve('.');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');

test('release workflow configura attestation, updater json y checksums', () => {
  const workflow = read('.github/workflows/release.yml');
  assert.match(workflow, /actions\/attest@v4/);
  assert.match(workflow, /generate-updater-json\.mjs/);
  assert.match(workflow, /generate-checksums\.mjs/);
  assert.match(workflow, /releaseAssetNamePattern/);
  assert.match(workflow, /com\.aatv\.midway/);
});

test('tauri release configs y renderer separan stable de beta sin placeholders quemados', () => {
  const releaseConfig = read('src-tauri/tauri.release.conf.json');
  const betaConfig = read('src-tauri/tauri.beta.conf.json');
  const renderScript = read('scripts/release/render-tauri-config.mjs');

  assert.match(releaseConfig, /createUpdaterArtifacts/);
  assert.match(releaseConfig, /installMode/);
  assert.match(betaConfig, /"updater"/);
  assert.match(renderScript, /latest\.json/);
  assert.match(renderScript, /latest-beta\.json/);
  assert.match(renderScript, /com\.aatv\.midway/);
  assert.doesNotMatch(releaseConfig, /OWNER\/REPO|REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY/);
  assert.doesNotMatch(betaConfig, /OWNER\/REPO/);
});

test('documentación de distribución enumera secrets y canales', () => {
  const docs = read('docs/distribution.md');
  assert.match(docs, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(docs, /APPLE_CERTIFICATE/);
  assert.match(docs, /latest-beta\.json/);
});

test('repositorio incluye licencia, gitignore y config base lista para publicar', () => {
  const gitignore = read('.gitignore');
  const license = read('LICENSE');
  const tauriConfig = JSON.parse(read('src-tauri/tauri.conf.json'));

  assert.match(gitignore, /node_modules\//);
  assert.match(gitignore, /src-tauri\/target\//);
  assert.match(license, /MIT License/);
  assert.equal(tauriConfig.identifier, 'com.aatv.midway');
  assert.ok(!('homepage' in tauriConfig.bundle));
});
