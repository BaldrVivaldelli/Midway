import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

// ---------------------------------------------------------------------------
// Generación de latest.json / latest-beta.json (Fase 7, Tarea 15.3)
//
// Este script es agnóstico del empaquetador: lee los assets subidos al release
// de GitHub y produce el mismo esquema de manifiesto (`version`, `notes`,
// `pub_date`, `platforms` con clave `{os}-{arch}`) que consume el updater de
// `midway-desktop` (ver midway-desktop/src/updater.rs: `UpdateManifest` /
// `UpdatePlatformEntry`).
//
// La migración Tauri -> cargo-packager solo cambia los NOMBRES de archivo de
// los instaladores, por lo que aquí únicamente se ajustan los patrones de
// coincidencia a los que produce `cargo-packager` (ver config en
// midway-desktop/Cargo.toml [package.metadata.packager], Tarea 15.1):
//
//   AppImage : {main_binary_name}_{version}_{arch}.AppImage   (arch: x86_64)
//   deb      : {main_binary_name}_{version}_{arch}.deb        (arch: amd64)
//   NSIS     : {main_binary_name}_{version}_{arch}-setup.exe  (arch: x64)
//   WiX/MSI  : {main_binary_name}_{version}_{arch}_{lang}.msi (arch: x64)
//
// donde `main_binary_name` = "midway-desktop". Los patrones se anclan al
// sufijo (arch + extensión), no al prefijo del producto, para tolerar cambios
// de nombre de producto entre canales stable/beta.
//
// Sobre el campo `signature`: el updater de midway-desktop verifica la
// integridad contra `SHA256SUMS.txt` (por nombre de archivo), no contra una
// firma minisign; `signature` se reinterpreta como referencia al checksum
// (ver Data Models > Manifest del updater). Por eso, si `cargo-packager`
// generó artefactos de updater firmados (`{artefacto}.sig`) se usa esa firma;
// en caso contrario se rellena con el SHA256 (hex minúsculas) del artefacto,
// manteniendo el esquema estable y el campo siempre poblado.
// ---------------------------------------------------------------------------

function usage() {
  console.error(
    'Uso: node scripts/release/generate-updater-json.mjs --release-json out-release.json --assets-dir release-assets --repo owner/repo --channel stable|beta --output latest.json'
  );
  process.exit(1);
}

const args = process.argv.slice(2);
const options = new Map();
for (let i = 0; i < args.length; i += 2) {
  options.set(args[i], args[i + 1]);
}

const releaseJsonPath = options.get('--release-json');
const assetsDir = options.get('--assets-dir');
const repo = options.get('--repo');
const channel = options.get('--channel') ?? 'stable';
const output = options.get('--output') ?? (channel === 'beta' ? 'latest-beta.json' : 'latest.json');

if (!releaseJsonPath || !assetsDir || !repo) usage();

const release = JSON.parse(fs.readFileSync(releaseJsonPath, 'utf8'));
const assets = Array.isArray(release.assets) ? release.assets : [];
const downloadedFiles = fs.readdirSync(assetsDir);
const tag = release.tagName ?? release.tag_name;
if (!tag) {
  throw new Error('No se encontró tagName en el JSON del release.');
}

function stripIndent(value) {
  return String(value ?? '').trim().replace(/\r\n/g, '\n');
}

function downloadUrl(name) {
  return `https://github.com/${repo}/releases/download/${tag}/${name}`;
}

// Devuelve el nombre del primer asset subido al release que coincide con
// alguno de los patrones, en orden de preferencia (p. ej. NSIS antes que MSI).
function uploadedAssetName(patterns) {
  for (const pattern of patterns) {
    const asset = assets.find((entry) => pattern.test(entry.name));
    if (asset) return asset.name;
  }
  return null;
}

// Localiza un archivo descargado por nombre exacto o por su basename.
function findLocalFile(name) {
  return (
    downloadedFiles.find((entry) => entry === name) ??
    downloadedFiles.find((entry) => path.basename(entry) === path.basename(name)) ??
    null
  );
}

// Resuelve el valor de `signature` para un artefacto concreto:
//   1) firma minisign `{artefacto}.sig` si cargo-packager la generó, o
//   2) el SHA256 (hex minúsculas) del artefacto como referencia al checksum
//      publicado en SHA256SUMS.txt (formato que consume midway-desktop).
function signatureForAsset(assetName) {
  const sigFile = findLocalFile(`${assetName}.sig`);
  if (sigFile) {
    return fs.readFileSync(path.join(assetsDir, sigFile), 'utf8').trim();
  }

  const artifactFile = findLocalFile(assetName);
  if (artifactFile) {
    const hash = crypto.createHash('sha256');
    hash.update(fs.readFileSync(path.join(assetsDir, artifactFile)));
    return hash.digest('hex');
  }

  return null;
}

// Construye la entrada de plataforma `{os}-{arch}` a partir de los patrones de
// nombre del artefacto empaquetado por cargo-packager para esa plataforma.
function buildEntry(platformKey, assetPatterns) {
  const baseAsset = uploadedAssetName(assetPatterns);
  if (!baseAsset) return null;

  const signature = signatureForAsset(baseAsset);
  if (!signature) return null;

  return {
    [platformKey]: {
      url: downloadUrl(baseAsset),
      signature
    }
  };
}

// Solo se empaquetan Windows y Linux x86_64 (la migración descarta macOS).
// Las claves de plataforma usan la arquitectura de Rust (`x86_64`), igual que
// `current_platform_key()` en midway-desktop/src/updater.rs, con
// independencia del token de arquitectura que use el nombre de archivo
// (x86_64/amd64 en Linux, x64 en Windows).
const platforms = Object.assign(
  {},
  buildEntry('linux-x86_64', [
    // AppImage es el artefacto auto-actualizable en Linux.
    /_x86_64\.AppImage$/i,
    /_amd64\.AppImage$/i,
    /\.AppImage$/i
  ]) ?? {},
  buildEntry('windows-x86_64', [
    // Se prefiere NSIS (setup.exe) sobre MSI para el updater.
    /_x64-setup\.exe$/i,
    /_x86_64-setup\.exe$/i,
    /-setup\.exe$/i,
    /_x64_[^/]*\.msi$/i,
    /\.msi$/i
  ]) ?? {}
);

if (Object.keys(platforms).length === 0) {
  throw new Error('No pude inferir assets del updater a partir de los archivos descargados del release.');
}

const payload = {
  version: String(tag).replace(/^v/, ''),
  notes: stripIndent(release.body) || `Release ${tag} (${channel})`,
  pub_date: new Date().toISOString(),
  platforms
};

fs.writeFileSync(output, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
console.log(`Updater manifest escrito en ${output}`);
