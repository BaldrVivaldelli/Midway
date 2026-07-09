import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

// ---------------------------------------------------------------------------
// Fase 7, Tarea 15.6: smoke tests del pipeline de CI para instaladores y
// artefactos del updater.
//
// Estos tests verifican, con 1-2 ejemplos representativos, que:
//   - el workflow de release genera instaladores para Windows y Linux x86_64
//     (Requisito 8.2), y
//   - el pipeline produce, a partir de esos instaladores, los artefactos que
//     consume el updater de midway-desktop: latest.json / latest-beta.json
//     (Requisito 8.3) y SHA256SUMS.txt (Requisito 8.4).
//
// No se ejecuta `cargo packager` (requiere toolchain nativa por plataforma);
// en su lugar se parte de instaladores representativos con los nombres que
// produce cargo-packager y se ejercita la cadena real de scripts del pipeline
// (generate-updater-json.mjs + generate-checksums.mjs) tal como los invoca el
// job `metadata` de .github/workflows/release.yml.
// ---------------------------------------------------------------------------

const root = path.resolve('.');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');
const sha256Hex = (buffer) => crypto.createHash('sha256').update(buffer).digest('hex');

// Instaladores representativos (1 por plataforma) con los nombres que
// cargo-packager produce para midway-desktop x86_64.
const linuxInstaller = 'midway-desktop_0.2.0_x86_64.AppImage';
const windowsInstaller = 'midway-desktop_0.2.0_x64-setup.exe';

function runUpdaterJson({ releaseJsonPath, assetsDir, channel, output }) {
  execFileSync(process.execPath, [
    path.join(root, 'scripts/release/generate-updater-json.mjs'),
    '--release-json', releaseJsonPath,
    '--assets-dir', assetsDir,
    '--repo', 'BaldrVivaldelli/Midway',
    '--channel', channel,
    '--output', output
  ]);
}

function runChecksums({ assetsDir, output }) {
  execFileSync(process.execPath, [
    path.join(root, 'scripts/release/generate-checksums.mjs'),
    assetsDir,
    output
  ]);
}

// Prepara un directorio de release con los instaladores Windows/Linux, tal
// como quedarían tras descargar los assets del draft en el job `metadata`.
function withInstallerFixture(run) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'midway-ci-smoke-'));
  try {
    const assetsDir = path.join(dir, 'release-assets');
    fs.mkdirSync(assetsDir, { recursive: true });

    const contents = {
      linux: Buffer.from('fake-linux-appimage-payload'),
      windows: Buffer.from('fake-windows-nsis-setup-payload')
    };
    fs.writeFileSync(path.join(assetsDir, linuxInstaller), contents.linux);
    fs.writeFileSync(path.join(assetsDir, windowsInstaller), contents.windows);

    const releaseJsonPath = path.join(dir, 'out-release.json');
    fs.writeFileSync(
      releaseJsonPath,
      JSON.stringify({
        tagName: 'v0.2.0',
        body: 'Release de prueba (smoke)',
        assets: [{ name: linuxInstaller }, { name: windowsInstaller }]
      }),
      'utf8'
    );

    run({ dir, assetsDir, releaseJsonPath, contents });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test('pipeline de release genera instaladores Windows y Linux x86_64', () => {
  const workflow = read('.github/workflows/release.yml');

  // El job `build` corre sobre la matriz Windows/Linux x86_64 (sin macOS) y
  // empaqueta midway-desktop con cargo-packager.
  assert.match(workflow, /platform_key:\s*linux-x86_64/);
  assert.match(workflow, /platform_key:\s*windows-x86_64/);
  assert.match(workflow, /runner:\s*ubuntu-22\.04/);
  assert.match(workflow, /runner:\s*windows-latest/);
  assert.match(workflow, /cargo packager/);
  assert.match(workflow, /packager_formats:\s*appimage deb/);
  assert.match(workflow, /packager_formats:\s*nsis wix/);

  // Los instaladores se recolectan por extensión para ambas plataformas.
  assert.match(workflow, /-name '\*\.AppImage'/);
  assert.match(workflow, /-name '\*-setup\.exe'/);

  // No se empaqueta macOS.
  assert.doesNotMatch(workflow, /macos-latest|darwin/);
});

test('smoke: instaladores -> latest.json / latest-beta.json / SHA256SUMS.txt', () => {
  withInstallerFixture(({ dir, assetsDir, releaseJsonPath, contents }) => {
    // 1) A partir de los instaladores, generar el manifiesto del updater para
    //    ambos canales (Requisito 8.3).
    const stableManifestPath = path.join(dir, 'latest.json');
    const betaManifestPath = path.join(dir, 'latest-beta.json');
    runUpdaterJson({ releaseJsonPath, assetsDir, channel: 'stable', output: stableManifestPath });
    runUpdaterJson({ releaseJsonPath, assetsDir, channel: 'beta', output: betaManifestPath });

    assert.ok(fs.existsSync(stableManifestPath), 'debe generarse latest.json');
    assert.ok(fs.existsSync(betaManifestPath), 'debe generarse latest-beta.json');

    for (const manifestPath of [stableManifestPath, betaManifestPath]) {
      const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
      assert.equal(manifest.version, '0.2.0');
      // Ambas plataformas presentes y apuntando a su instalador.
      assert.deepEqual(
        Object.keys(manifest.platforms).sort(),
        ['linux-x86_64', 'windows-x86_64']
      );
      assert.ok(manifest.platforms['linux-x86_64'].url.endsWith(linuxInstaller));
      assert.ok(manifest.platforms['windows-x86_64'].url.endsWith(windowsInstaller));
    }

    // 2) Emular el job `metadata`: el manifiesto del canal se copia junto a los
    //    instaladores antes de calcular los checksums (Requisito 8.4).
    fs.copyFileSync(stableManifestPath, path.join(assetsDir, 'latest.json'));
    const checksumsPath = path.join(dir, 'SHA256SUMS.txt');
    runChecksums({ assetsDir, output: checksumsPath });

    assert.ok(fs.existsSync(checksumsPath), 'debe generarse SHA256SUMS.txt');
    const checksums = fs.readFileSync(checksumsPath, 'utf8').trim().split('\n');

    // Un checksum por artefacto empaquetado + el manifiesto del updater.
    const entriesByFile = new Map(
      checksums.map((line) => {
        const [hash, file] = line.split('  ');
        return [file, hash];
      })
    );
    assert.equal(entriesByFile.get(linuxInstaller), sha256Hex(contents.linux));
    assert.equal(entriesByFile.get(windowsInstaller), sha256Hex(contents.windows));
    assert.ok(entriesByFile.has('latest.json'), 'SHA256SUMS.txt debe incluir latest.json');

    // 3) Consistencia end-to-end: el `signature` del manifiesto (referencia al
    //    checksum) coincide con la entrada de SHA256SUMS.txt del mismo
    //    instalador, que es lo que el updater verifica antes de instalar.
    const stableManifest = JSON.parse(fs.readFileSync(stableManifestPath, 'utf8'));
    assert.equal(
      stableManifest.platforms['linux-x86_64'].signature,
      entriesByFile.get(linuxInstaller)
    );
    assert.equal(
      stableManifest.platforms['windows-x86_64'].signature,
      entriesByFile.get(windowsInstaller)
    );
  });
});
