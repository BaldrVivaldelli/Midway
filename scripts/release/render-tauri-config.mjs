#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const args = new Map();
for (let index = 2; index < process.argv.length; index += 1) {
  const token = process.argv[index];
  if (!token.startsWith('--')) continue;
  const next = process.argv[index + 1];
  args.set(token.slice(2), next && !next.startsWith('--') ? next : 'true');
  if (next && !next.startsWith('--')) index += 1;
}

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), 'utf8'));
}

function writeJson(relativePath, value) {
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

const baseTauriConfig = readJson('src-tauri/tauri.conf.json');

function pick(...values) {
  for (const value of values) {
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
}

const channel = pick(args.get('channel'), process.env.MIDWAY_RELEASE_CHANNEL, 'stable');
const repository = pick(
  args.get('repo'),
  process.env.MIDWAY_GITHUB_REPOSITORY,
  process.env.GITHUB_REPOSITORY
);
const pubkey = pick(
  args.get('pubkey'),
  process.env.MIDWAY_UPDATER_PUBLIC_KEY,
  process.env.TAURI_UPDATER_PUBLIC_KEY
);
const identifier = pick(
  args.get('identifier'),
  process.env.MIDWAY_APP_IDENTIFIER,
  process.env.MIDWAY_BUNDLE_IDENTIFIER,
  baseTauriConfig.identifier,
  'com.aatv.midway'
);
const productName = pick(
  args.get('product-name'),
  process.env.MIDWAY_PRODUCT_NAME,
  'Midway'
);
const publisher = pick(
  args.get('publisher'),
  process.env.MIDWAY_PUBLISHER,
  'Midway'
);
const homepage = pick(
  args.get('homepage'),
  process.env.MIDWAY_HOMEPAGE,
  baseTauriConfig.bundle?.homepage,
  repository ? `https://github.com/${repository}` : null
);
const betaProductName = pick(
  args.get('beta-product-name'),
  process.env.MIDWAY_BETA_PRODUCT_NAME,
  `${productName} Beta`
);

const missing = [];
if (!repository) missing.push('repo / MIDWAY_GITHUB_REPOSITORY');
if (!pubkey) missing.push('pubkey / MIDWAY_UPDATER_PUBLIC_KEY');
if (!identifier) missing.push('identifier / MIDWAY_APP_IDENTIFIER');
if (!homepage) missing.push('homepage / MIDWAY_HOMEPAGE');
if (missing.length > 0) {
  throw new Error(`Faltan variables para generar la config de release: ${missing.join(', ')}`);
}

const releaseTemplate = readJson('src-tauri/tauri.release.conf.json');
const betaTemplate = readJson('src-tauri/tauri.beta.conf.json');

const stable = {
  ...releaseTemplate,
  identifier,
  productName,
  bundle: {
    ...(releaseTemplate.bundle ?? {}),
    homepage,
    publisher
  },
  plugins: {
    ...(releaseTemplate.plugins ?? {}),
    updater: {
      ...((releaseTemplate.plugins ?? {}).updater ?? {}),
      pubkey,
      endpoints: [
        `https://github.com/${repository}/releases/latest/download/latest.json`
      ],
      windows: {
        installMode: 'passive',
        ...(((releaseTemplate.plugins ?? {}).updater ?? {}).windows ?? {})
      }
    }
  }
};

const beta = {
  ...betaTemplate,
  productName: betaProductName,
  identifier: `${identifier}.beta`,
  bundle: {
    homepage,
    publisher
  },
  plugins: {
    ...(betaTemplate.plugins ?? {}),
    updater: {
      ...((betaTemplate.plugins ?? {}).updater ?? {}),
      endpoints: [
        `https://github.com/${repository}/releases/latest/download/latest-beta.json`
      ]
    }
  }
};

writeJson('src-tauri/tauri.release.generated.json', stable);
writeJson('src-tauri/tauri.beta.generated.json', beta);

const summary = {
  channel,
  repository,
  identifier: channel === 'beta' ? beta.identifier : stable.identifier,
  productName: channel === 'beta' ? beta.productName : stable.productName,
  homepage,
  publisher,
  endpoints:
    channel === 'beta'
      ? beta.plugins.updater.endpoints
      : stable.plugins.updater.endpoints
};
writeJson('out/release-config-summary.json', summary);
console.log(JSON.stringify(summary, null, 2));
