import fs from 'node:fs';
import path from 'node:path';

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

function uploadedAssetName(patterns) {
  for (const pattern of patterns) {
    const asset = assets.find((entry) => pattern.test(entry.name));
    if (asset) return asset.name;
  }
  return null;
}

function localSignature(patterns) {
  for (const pattern of patterns) {
    const file = downloadedFiles.find((entry) => pattern.test(entry));
    if (file) {
      return fs.readFileSync(path.join(assetsDir, file), 'utf8').trim();
    }
  }
  return null;
}

function buildEntry(os, arch) {
  if (os === 'linux') {
    const baseAsset = uploadedAssetName([
      new RegExp(`_${os}_${arch}\\.AppImage$`, 'i')
    ]);
    const signature = localSignature([
      new RegExp(`_${os}_${arch}\\.AppImage\\.sig$`, 'i'),
      /\.AppImage\.sig$/i
    ]);
    if (!baseAsset || !signature) return null;
    return {
      [`${os}-${arch}`]: {
        url: downloadUrl(baseAsset),
        signature
      }
    };
  }

  if (os === 'windows') {
    const baseAsset = uploadedAssetName([
      new RegExp(`_${os}_${arch}_setup\\.exe$`, 'i'),
      new RegExp(`_${os}_${arch}\\.msi$`, 'i')
    ]);
    const signature = localSignature([
      /_setup\.exe\.sig$/i,
      /\.msi\.sig$/i
    ]);
    if (!baseAsset || !signature) return null;
    return {
      [`${os}-${arch}`]: {
        url: downloadUrl(baseAsset),
        signature
      }
    };
  }

  if (os === 'darwin') {
    const baseAsset = uploadedAssetName([
      new RegExp(`_(darwin|macos)_${arch}\\.app\\.tar\\.gz$`, 'i')
    ]);
    const signature = localSignature([
      new RegExp(`_(darwin|macos)_${arch}\\.app\\.tar\\.gz\\.sig$`, 'i'),
      /\.app\.tar\.gz\.sig$/i
    ]);
    if (!baseAsset || !signature) return null;
    return {
      [`${os}-${arch}`]: {
        url: downloadUrl(baseAsset),
        signature
      }
    };
  }

  return null;
}

const platforms = Object.assign(
  {},
  buildEntry('linux', 'x86_64') ?? {},
  buildEntry('windows', 'x86_64') ?? {},
  buildEntry('darwin', 'x86_64') ?? {},
  buildEntry('darwin', 'aarch64') ?? {}
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
