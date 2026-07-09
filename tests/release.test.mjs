import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const root = path.resolve('.');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');

const sha256Hex = (buffer) => crypto.createHash('sha256').update(buffer).digest('hex');

test('release workflow configura attestation, updater json y checksums', () => {
  const workflow = read('.github/workflows/release.yml');
  assert.match(workflow, /actions\/attest@v4/);
  assert.match(workflow, /generate-updater-json\.mjs/);
  assert.match(workflow, /generate-checksums\.mjs/);
  // Tarea 15.4: el empaquetado con `tauri-apps/tauri-action` (y su input
  // `releaseAssetNamePattern`) se reemplazó por la invocación directa de
  // `cargo-packager` sobre midway-desktop, pinneado a 0.11.8. La aserción
  // sobre `releaseAssetNamePattern` quedó obsoleta y se sustituye por la del
  // nuevo mecanismo de empaquetado.
  assert.match(workflow, /cargo install cargo-packager --version =0\.11\.8 --locked/);
  assert.match(workflow, /cargo packager/);
  assert.match(workflow, /render-packager-config\.mjs/);
  assert.doesNotMatch(workflow, /tauri-apps\/tauri-action|releaseAssetNamePattern/);
  assert.match(workflow, /com\.aatv\.midway/);
  assert.match(workflow, /windows-latest/);
  assert.match(workflow, /ubuntu-22\.04/);
  assert.doesNotMatch(workflow, /macos-latest|darwin|APPLE_CERTIFICATE/);
});

test('release workflow detiene el pipeline ante fallo de empaquetado sin publicar artefactos parciales (8.6)', () => {
  const workflow = read('.github/workflows/release.yml');

  // La matriz de empaquetado usa fail-fast: true, de modo que un fallo en
  // cualquier plataforma cancela las patas restantes y detiene el pipeline.
  assert.match(workflow, /fail-fast:\s*true/);
  assert.doesNotMatch(workflow, /fail-fast:\s*false/);

  // El paso de empaquetado es identificable y hay un paso dedicado que reporta
  // la plataforma afectada y la causa cuando cargo-packager falla.
  assert.match(workflow, /id:\s*package/);
  assert.match(workflow, /name:\s*Report packaging failure/);
  assert.match(workflow, /steps\.package\.outcome == 'failure'/);
  assert.match(workflow, /::error title=Empaquetado fallido/);

  // Salvaguarda: si no se produjo ningún instalador, el pipeline se detiene en
  // lugar de continuar hacia la publicación.
  assert.match(workflow, /name:\s*Verify installers were produced/);

  // Los instaladores se dejan en staging (artefacto de workflow); NO se suben al
  // release desde la matriz. El job build ya no tiene permisos de escritura.
  assert.match(workflow, /actions\/upload-artifact@v4/);
  assert.match(workflow, /actions\/download-artifact@v4/);

  // La publicación al release ocurre en un job aparte que depende del éxito de
  // TODA la matriz (needs: build), y los metadatos del updater dependen de esa
  // publicación (needs: publish). Así, un fallo parcial nunca publica nada.
  assert.match(workflow, /publish:\s*\n\s*needs:\s*build/);
  assert.match(workflow, /metadata:\s*\n\s*needs:\s*publish/);
});

test('el renderer de cargo-packager separa stable de beta sin placeholders quemados', () => {
  // Tras la Fase 8 de la migración a iced, los antiguos
  // src-tauri/tauri.release.conf.json y tauri.beta.conf.json fueron eliminados.
  // La config base de empaquetado vive ahora en midway-desktop/Cargo.toml
  // ([package.metadata.packager]) como fuente única de verdad, y el renderer de
  // cargo-packager aplica los overrides por canal (stable/beta).
  const renderScript = read('scripts/release/render-packager-config.mjs');
  const packagerBaseConfig = read('midway-desktop/Cargo.toml');

  // El renderer genera config por canal (stable/beta) y mapea cada canal a su
  // manifiesto de updater (latest.json / latest-beta.json).
  assert.match(renderScript, /package\.metadata\.packager/);
  assert.match(renderScript, /packager\.stable\.generated\.json/);
  assert.match(renderScript, /packager\.beta\.generated\.json/);
  assert.match(renderScript, /latest\.json/);
  assert.match(renderScript, /latest-beta\.json/);
  assert.match(renderScript, /com\.aatv\.midway/);
  // La config base (identifier) vive en Cargo.toml, no en placeholders quemados.
  assert.match(packagerBaseConfig, /\[package\.metadata\.packager\]/);
  assert.match(packagerBaseConfig, /identifier = "com\.aatv\.midway"/);
  assert.doesNotMatch(renderScript, /OWNER\/REPO|REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY/);
});

test('documentación de distribución enumera secrets y canales', () => {
  const docs = read('docs/distribution.md');
  assert.match(docs, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(docs, /latest-beta\.json/);
  assert.match(docs, /Windows/);
  assert.match(docs, /Linux/);
  assert.doesNotMatch(docs, /APPLE_CERTIFICATE|macOS/);
});

test('repositorio incluye licencia, gitignore y config base lista para publicar', () => {
  const gitignore = read('.gitignore');
  const license = read('LICENSE');
  // Tras la Fase 8, la config base de empaquetado ya no vive en
  // src-tauri/tauri.conf.json sino en midway-desktop/Cargo.toml
  // ([package.metadata.packager]).
  const packagerBaseConfig = read('midway-desktop/Cargo.toml');

  assert.match(gitignore, /node_modules\//);
  // Los configs por canal generados por el renderer de cargo-packager quedan
  // fuera del control de versiones.
  assert.match(gitignore, /midway-desktop\/packager\.stable\.generated\.json/);
  assert.match(gitignore, /midway-desktop\/packager\.beta\.generated\.json/);
  assert.match(license, /MIT License/);
  assert.match(packagerBaseConfig, /identifier = "com\.aatv\.midway"/);
});

// ---------------------------------------------------------------------------
// Fase 7, Tarea 15.3: los scripts de release operan sobre los artefactos de
// cargo-packager, preservando el esquema de latest.json / SHA256SUMS.txt que
// consume el updater de midway-desktop.
// ---------------------------------------------------------------------------

// Nombres de archivo tal como los produce cargo-packager para midway-desktop
// (binario "midway-desktop"): AppImage/deb en Linux, NSIS/MSI en Windows.
const packagerArtifacts = {
  appImage: 'midway-desktop_0.2.0_x86_64.AppImage',
  deb: 'midway-desktop_0.2.0_amd64.deb',
  nsis: 'midway-desktop_0.2.0_x64-setup.exe',
  msi: 'midway-desktop_0.2.0_x64_en-US.msi'
};

function withReleaseFixture(run) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'midway-release-'));
  try {
    const assetsDir = path.join(dir, 'release-assets');
    fs.mkdirSync(assetsDir, { recursive: true });

    const contents = {};
    for (const [key, name] of Object.entries(packagerArtifacts)) {
      const body = Buffer.from(`artifact:${name}`);
      fs.writeFileSync(path.join(assetsDir, name), body);
      contents[key] = body;
    }

    const releaseJsonPath = path.join(dir, 'out-release.json');
    fs.writeFileSync(
      releaseJsonPath,
      JSON.stringify({
        tagName: 'v0.2.0',
        body: 'Notas de la release',
        assets: Object.values(packagerArtifacts).map((name) => ({ name }))
      }),
      'utf8'
    );

    run({ dir, assetsDir, releaseJsonPath, contents });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test('generate-updater-json mapea artefactos de cargo-packager al esquema del updater', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath, contents }) => {
    const outputPath = path.join(dir, 'latest.json');
    execFileSync(process.execPath, [
      path.join(root, 'scripts/release/generate-updater-json.mjs'),
      '--release-json', releaseJsonPath,
      '--assets-dir', assetsDir,
      '--repo', 'BaldrVivaldelli/Midway',
      '--channel', 'stable',
      '--output', outputPath
    ]);

    const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'));

    // Esquema: version sin prefijo `v`, platforms con clave {os}-{arch}.
    assert.equal(manifest.version, '0.2.0');
    assert.equal(manifest.notes, 'Notas de la release');
    assert.ok(typeof manifest.pub_date === 'string' && manifest.pub_date.length > 0);
    assert.deepEqual(Object.keys(manifest.platforms).sort(), ['linux-x86_64', 'windows-x86_64']);

    // Linux -> AppImage; Windows -> NSIS (setup.exe) preferido sobre MSI.
    assert.ok(manifest.platforms['linux-x86_64'].url.endsWith(packagerArtifacts.appImage));
    assert.ok(manifest.platforms['windows-x86_64'].url.endsWith(packagerArtifacts.nsis));
    assert.match(
      manifest.platforms['linux-x86_64'].url,
      /^https:\/\/github\.com\/BaldrVivaldelli\/Midway\/releases\/download\/v0\.2\.0\//
    );

    // signature se reinterpreta como referencia al checksum: SHA256 hex del
    // artefacto (sin `.sig` de minisign disponible).
    assert.equal(manifest.platforms['linux-x86_64'].signature, sha256Hex(contents.appImage));
    assert.equal(manifest.platforms['windows-x86_64'].signature, sha256Hex(contents.nsis));
  });
});

test('generate-updater-json usa la firma minisign .sig cuando cargo-packager la genera', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath }) => {
    const sigValue = 'untrusted comment: minisign signature\nRWQ...\n';
    fs.writeFileSync(path.join(assetsDir, `${packagerArtifacts.appImage}.sig`), sigValue, 'utf8');

    const outputPath = path.join(dir, 'latest.json');
    execFileSync(process.execPath, [
      path.join(root, 'scripts/release/generate-updater-json.mjs'),
      '--release-json', releaseJsonPath,
      '--assets-dir', assetsDir,
      '--repo', 'BaldrVivaldelli/Midway',
      '--channel', 'stable',
      '--output', outputPath
    ]);

    const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
    assert.equal(manifest.platforms['linux-x86_64'].signature, sigValue.trim());
  });
});

test('generate-updater-json respeta el canal beta (latest-beta.json)', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath }) => {
    const outputPath = path.join(dir, 'latest-beta.json');
    execFileSync(process.execPath, [
      path.join(root, 'scripts/release/generate-updater-json.mjs'),
      '--release-json', releaseJsonPath,
      '--assets-dir', assetsDir,
      '--repo', 'BaldrVivaldelli/Midway',
      '--channel', 'beta',
      '--output', outputPath
    ]);

    assert.ok(fs.existsSync(outputPath));
    const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
    assert.ok(manifest.platforms['linux-x86_64']);
    assert.ok(manifest.platforms['windows-x86_64']);
  });
});

test('generate-checksums produce el formato {hash}  {archivo} en hex minúsculas', () => {
  withReleaseFixture(({ dir, assetsDir, contents }) => {
    const outputPath = path.join(dir, 'SHA256SUMS.txt');
    execFileSync(process.execPath, [
      path.join(root, 'scripts/release/generate-checksums.mjs'),
      assetsDir,
      outputPath
    ]);

    const document = fs.readFileSync(outputPath, 'utf8');
    const lines = document.trim().split('\n');

    // Cada línea: 64 hex minúsculas + dos espacios + nombre de archivo.
    for (const line of lines) {
      assert.match(line, /^[0-9a-f]{64} {2}\S/);
    }

    // El checksum del AppImage coincide con el SHA256 real del artefacto.
    const appImageLine = lines.find((line) => line.endsWith(packagerArtifacts.appImage));
    assert.ok(appImageLine, 'SHA256SUMS.txt debe incluir el AppImage');
    assert.equal(appImageLine.split('  ')[0], sha256Hex(contents.appImage));
  });
});

// ---------------------------------------------------------------------------
// Fase 7, Tarea 15.5 (Requisito 8.6): un fallo de empaquetado en CUALQUIER
// plataforma detiene el pipeline y evita publicar artefactos parciales o
// corruptos. El release workflow logra esto con: (a) `fail-fast: true` en la
// matriz de build; (b) el job de build deja los instaladores en staging (no
// toca el release); (c) un job `publish` que depende del éxito de TODA la
// matriz sube al draft, tras verificar que los instaladores estén presentes;
// (d) el job `metadata` depende de `publish`, no del build directo.
// ---------------------------------------------------------------------------
test('release workflow detiene el pipeline ante fallo de empaquetado en alguna plataforma', () => {
  const workflow = read('.github/workflows/release.yml');

  // (a) La matriz de build usa fail-fast para cancelar el resto ante un fallo.
  assert.match(workflow, /fail-fast:\s*true/);

  // (b) El build NO publica directamente en el release: sólo deja staging.
  assert.match(workflow, /actions\/upload-artifact@v4/);
  assert.match(workflow, /if-no-files-found:\s*error/);

  // (c) Existe un job `publish` que depende del build completo (todas las
  //     plataformas) antes de subir instaladores al draft.
  assert.match(workflow, /^\s{2}publish:/m);
  assert.match(workflow, /^\s{2}build:/m);

  // (d) Los metadatos del updater dependen de `publish`, garantizando que sólo
  //     se generan cuando el release tiene el set completo de instaladores.
  assert.match(workflow, /metadata:\s*[\s\S]*?needs:\s*publish/);

  // El fallo de empaquetado se reporta identificando la plataforma afectada.
  assert.match(workflow, /platform_key/);
  assert.match(workflow, /Empaquetado fallido/);
});

// ---------------------------------------------------------------------------
// Fase 7, Tarea 15.6 (Requisitos 8.2, 8.3, 8.4): smoke tests del pipeline de
// CI para instaladores + artefactos del updater. Validan, a nivel de script y
// sin toolchains de plataforma reales, que el pipeline funciona de extremo a
// extremo y de forma consistente:
//
//   (8.2) por cada plataforma (Windows/Linux x86_64) se produce un instalador
//         y el manifiesto del updater lo referencia realmente;
//   (8.3) se generan latest.json / latest-beta.json con el esquema del updater;
//   (8.4) SHA256SUMS.txt cubre TODOS los instaladores producidos MÁS el propio
//         manifiesto del updater, con hashes que coinciden con el contenido.
//
// El "wiring" del workflow (release.yml) se verifica por separado para
// garantizar que lo que la matriz PRODUCE (formatos/extensiones) es exactamente
// lo que los jobs `publish`/`metadata` CONSUMEN (atestación + updater json +
// checksums), cerrando el lazo produce/consume.
// ---------------------------------------------------------------------------

// Reproduce el paso `metadata` del workflow: genera el manifiesto del updater a
// partir de los assets del release y luego produce SHA256SUMS.txt sobre esos
// mismos assets CON el manifiesto ya copiado dentro (tal como hace release.yml:
// `cp "${output}" release-assets/` antes de invocar generate-checksums). Así el
// checksum cubre instaladores + manifiesto, igual que en producción.
function runMetadataPipeline({ dir, assetsDir, releaseJsonPath, channel }) {
  const manifestName = channel === 'beta' ? 'latest-beta.json' : 'latest.json';
  const manifestPath = path.join(dir, manifestName);

  execFileSync(process.execPath, [
    path.join(root, 'scripts/release/generate-updater-json.mjs'),
    '--release-json', releaseJsonPath,
    '--assets-dir', assetsDir,
    '--repo', 'BaldrVivaldelli/Midway',
    '--channel', channel,
    '--output', manifestPath
  ]);

  // El workflow copia el manifiesto al directorio de assets antes de calcular
  // los checksums, de modo que SHA256SUMS.txt también cubre el manifiesto.
  fs.copyFileSync(manifestPath, path.join(assetsDir, manifestName));

  const sumsPath = path.join(dir, 'SHA256SUMS.txt');
  execFileSync(process.execPath, [
    path.join(root, 'scripts/release/generate-checksums.mjs'),
    assetsDir,
    sumsPath
  ]);

  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const sums = fs.readFileSync(sumsPath, 'utf8').trim().split('\n');
  return { manifestName, manifest, sums };
}

// Convierte SHA256SUMS.txt en un mapa {basename -> hash} para aserciones.
function checksumMap(lines) {
  const map = new Map();
  for (const line of lines) {
    const [hash, ...rest] = line.split('  ');
    map.set(rest.join('  ').trim(), hash);
  }
  return map;
}

test('smoke: el manifiesto del updater referencia un instalador real por plataforma (8.2, 8.3)', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath }) => {
    const { manifest } = runMetadataPipeline({ dir, assetsDir, releaseJsonPath, channel: 'stable' });

    // (8.2 + 8.3) Ambas plataformas soportadas están presentes en el manifiesto.
    assert.deepEqual(Object.keys(manifest.platforms).sort(), ['linux-x86_64', 'windows-x86_64']);

    // El artefacto referenciado por cada plataforma EXISTE realmente entre los
    // instaladores producidos por cargo-packager (cierra "generar" -> "referenciar").
    for (const [platformKey, entry] of Object.entries(manifest.platforms)) {
      const referenced = path.basename(new URL(entry.url).pathname);
      assert.ok(
        fs.existsSync(path.join(assetsDir, referenced)),
        `${platformKey}: el manifiesto referencia ${referenced}, que debe existir como instalador producido`
      );
    }

    // Linux se auto-actualiza vía AppImage; Windows vía el instalador NSIS.
    assert.ok(manifest.platforms['linux-x86_64'].url.endsWith(packagerArtifacts.appImage));
    assert.ok(manifest.platforms['windows-x86_64'].url.endsWith(packagerArtifacts.nsis));
  });
});

test('smoke: SHA256SUMS.txt cubre todos los instaladores y el manifiesto del updater (8.4)', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath, contents }) => {
    const { manifestName, sums } = runMetadataPipeline({ dir, assetsDir, releaseJsonPath, channel: 'stable' });
    const map = checksumMap(sums);

    // (8.4) Hay una entrada por CADA instalador producido y su hash coincide con
    // el contenido real del artefacto.
    for (const [key, name] of Object.entries(packagerArtifacts)) {
      assert.ok(map.has(name), `SHA256SUMS.txt debe incluir el instalador ${name}`);
      assert.equal(map.get(name), sha256Hex(contents[key]), `hash incorrecto para ${name}`);
    }

    // (8.4 + 8.3) El propio manifiesto del updater también queda cubierto por el
    // checksum, de modo que el updater puede verificar su integridad.
    assert.ok(map.has(manifestName), `SHA256SUMS.txt debe incluir el manifiesto ${manifestName}`);
    assert.equal(
      map.get(manifestName),
      sha256Hex(fs.readFileSync(path.join(assetsDir, manifestName))),
      'hash del manifiesto no coincide con su contenido'
    );

    // Exactamente instaladores + manifiesto: ni de más (SHA256SUMS.txt se
    // excluye a sí mismo) ni de menos.
    assert.equal(map.size, Object.keys(packagerArtifacts).length + 1);
  });
});

test('smoke: canal beta produce latest-beta.json cubierto por checksums (8.3, 8.4)', () => {
  withReleaseFixture(({ dir, assetsDir, releaseJsonPath }) => {
    const { manifestName, manifest, sums } = runMetadataPipeline({
      dir,
      assetsDir,
      releaseJsonPath,
      channel: 'beta'
    });

    assert.equal(manifestName, 'latest-beta.json');
    assert.ok(manifest.platforms['linux-x86_64'] && manifest.platforms['windows-x86_64']);

    const map = checksumMap(sums);
    assert.ok(map.has('latest-beta.json'), 'SHA256SUMS.txt del canal beta debe cubrir latest-beta.json');
    // Cada instalador sigue estando cubierto en el canal beta.
    for (const name of Object.values(packagerArtifacts)) {
      assert.ok(map.has(name), `canal beta: falta ${name} en SHA256SUMS.txt`);
    }
  });
});

test('smoke: el workflow produce y consume los mismos artefactos de forma consistente (8.2, 8.3, 8.4)', () => {
  const workflow = read('.github/workflows/release.yml');

  // (8.2) La matriz produce instaladores para Windows y Linux x86_64.
  assert.match(workflow, /platform_key:\s*linux-x86_64/);
  assert.match(workflow, /platform_key:\s*windows-x86_64/);
  assert.match(workflow, /packager_formats:\s*appimage deb/);
  assert.match(workflow, /packager_formats:\s*nsis wix/);

  // Lo que la matriz RECOLECTA (Collect installers) debe cubrir las mismas
  // extensiones que produce cargo-packager y que consume el resto del pipeline.
  const collectStep = workflow.slice(workflow.indexOf('Collect installers'));
  for (const ext of ['*.AppImage', '*.deb', '*-setup.exe', '*.msi']) {
    assert.ok(collectStep.includes(ext), `Collect installers debe recolectar ${ext}`);
  }

  // El job `publish` atesta exactamente esas mismas extensiones (produce ->
  // consume consistente para la atestación).
  const attestStep = workflow.slice(workflow.indexOf('attest@v4'));
  for (const ext of ['dist/*.AppImage', 'dist/*.deb', 'dist/*-setup.exe', 'dist/*.msi']) {
    assert.ok(attestStep.includes(ext), `la atestación debe cubrir ${ext}`);
  }

  // (8.3 + 8.4) El job `metadata` consume los instaladores publicados para
  // generar el manifiesto del updater y los checksums, y sube ambos al release.
  assert.match(workflow, /generate-updater-json\.mjs/);
  assert.match(workflow, /generate-checksums\.mjs/);
  // El manifiesto se copia dentro de release-assets ANTES de calcular checksums,
  // de modo que SHA256SUMS.txt cubre también el manifiesto (invariante del test
  // de checksums de arriba).
  assert.match(workflow, /cp "\$\{\{ steps\.channel\.outputs\.output \}\}" release-assets\//);
  // Se limpian los metadatos previos antes de regenerarlos para no re-hashear
  // artefactos obsoletos.
  assert.match(workflow, /find release-assets \\\( -name 'latest\*\.json' -o -name 'SHA256SUMS\.txt' \\\) -delete/);
  // Se suben al release el manifiesto del canal (latest.json/latest-beta.json)
  // y el archivo de checksums.
  assert.match(workflow, /out\/SHA256SUMS\.txt/);
  assert.match(workflow, /latest-beta\.json/);
});
