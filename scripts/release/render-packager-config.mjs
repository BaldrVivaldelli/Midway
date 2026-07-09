#!/usr/bin/env node
// Genera la configuración de `cargo-packager` por canal (stable / beta) para
// `midway-desktop`, reemplazando al antiguo `render-tauri-config.mjs` que
// generaba overrides de `tauri.conf.json`.
//
// Fase 7 (Empaquetado y distribución) de la migración Tauri -> iced.
// Requisito 8.5: adaptar los scripts de `scripts/release/*` para construir y
// empaquetar `midway-desktop` en lugar de `midway` (Tauri).
//
// La configuración base vive en `midway-desktop/Cargo.toml`
// (`[package.metadata.packager]`, añadida por la tarea 15.1). Este script la lee
// como fuente única de verdad y produce dos configuraciones `packager.json`
// completas —una por canal— aplicando los overrides por canal que antes vivían
// en `tauri.release.conf.json` / `tauri.beta.conf.json`:
//
//   - identifier
//   - productName
//   - homepage
//   - publisher
//
// Salida:
//   - midway-desktop/packager.stable.generated.json
//   - midway-desktop/packager.beta.generated.json
//   - out/release-config-summary.json
//
// El pipeline de release invoca luego `cargo packager` con el archivo del canal
// correspondiente vía `-c/--config`.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const root = process.cwd();
const args = new Map();
for (let index = 2; index < process.argv.length; index += 1) {
  const token = process.argv[index];
  if (!token.startsWith('--')) continue;
  const next = process.argv[index + 1];
  args.set(token.slice(2), next && !next.startsWith('--') ? next : 'true');
  if (next && !next.startsWith('--')) index += 1;
}

function writeJson(relativePath, value) {
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function pick(...values) {
  for (const value of values) {
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
}

// Convierte recursivamente las claves kebab-case (como las escribe TOML en
// `[package.metadata.packager]`) a camelCase, que es el formato que espera el
// schema de `packager.json`. Solo se tocan las claves de objeto; los valores
// (rutas de íconos, URLs, etc.) quedan intactos.
function kebabToCamel(key) {
  return key.replace(/-([a-z0-9])/g, (_, char) => char.toUpperCase());
}

function camelizeKeysDeep(value) {
  if (Array.isArray(value)) {
    return value.map(camelizeKeysDeep);
  }
  if (value && typeof value === 'object') {
    const result = {};
    for (const [key, inner] of Object.entries(value)) {
      result[kebabToCamel(key)] = camelizeKeysDeep(inner);
    }
    return result;
  }
  return value;
}

// Lee la configuración base de `cargo-packager` desde el Cargo.toml de
// `midway-desktop` usando `cargo metadata` (fuente única de verdad, tarea 15.1).
function readBasePackagerConfig() {
  const manifestPath = path.join('midway-desktop', 'Cargo.toml');
  let raw;
  try {
    raw = execFileSync(
      'cargo',
      ['metadata', '--no-deps', '--format-version', '1', '--manifest-path', manifestPath],
      { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }
    );
  } catch (error) {
    throw new Error(
      `No se pudo ejecutar \`cargo metadata\` para leer la config base de cargo-packager: ${error.message}`
    );
  }

  let metadata;
  try {
    metadata = JSON.parse(raw);
  } catch (error) {
    throw new Error(`\`cargo metadata\` devolvió una salida no parseable: ${error.message}`);
  }

  const pkg = (metadata.packages ?? []).find((candidate) => candidate.name === 'midway-desktop');
  if (!pkg) {
    throw new Error('No se encontró el paquete `midway-desktop` en la salida de `cargo metadata`.');
  }

  const packager = pkg.metadata?.packager;
  if (!packager || typeof packager !== 'object') {
    throw new Error(
      'Falta `[package.metadata.packager]` en midway-desktop/Cargo.toml (esperado desde la tarea 15.1).'
    );
  }

  return camelizeKeysDeep(packager);
}

const base = readBasePackagerConfig();

const channel = pick(args.get('channel'), process.env.MIDWAY_RELEASE_CHANNEL, 'stable');
const repository = pick(
  args.get('repo'),
  process.env.MIDWAY_GITHUB_REPOSITORY,
  process.env.GITHUB_REPOSITORY
);
const identifier = pick(
  args.get('identifier'),
  process.env.MIDWAY_APP_IDENTIFIER,
  process.env.MIDWAY_BUNDLE_IDENTIFIER,
  base.identifier,
  'com.aatv.midway'
);
const productName = pick(
  args.get('product-name'),
  process.env.MIDWAY_PRODUCT_NAME,
  base.productName,
  'Midway'
);
const publisher = pick(
  args.get('publisher'),
  process.env.MIDWAY_PUBLISHER,
  base.publisher,
  'Midway'
);
const homepage = pick(
  args.get('homepage'),
  process.env.MIDWAY_HOMEPAGE,
  base.homepage,
  repository ? `https://github.com/${repository}` : null
);
const betaProductName = pick(
  args.get('beta-product-name'),
  process.env.MIDWAY_BETA_PRODUCT_NAME,
  `${productName} Beta`
);

const missing = [];
if (!repository) missing.push('repo / MIDWAY_GITHUB_REPOSITORY');
if (!identifier) missing.push('identifier / MIDWAY_APP_IDENTIFIER');
if (!homepage) missing.push('homepage / MIDWAY_HOMEPAGE');
if (missing.length > 0) {
  throw new Error(`Faltan variables para generar la config de release: ${missing.join(', ')}`);
}

// Canal estable: la config base con los overrides por canal aplicados.
const stable = {
  ...base,
  identifier,
  productName,
  homepage,
  publisher
};

// Canal beta: identifier y productName diferenciados para poder coexistir con la
// instalación estable, preservando el resto de la config base.
const beta = {
  ...base,
  identifier: `${identifier}.beta`,
  productName: betaProductName,
  homepage,
  publisher
};

writeJson('midway-desktop/packager.stable.generated.json', stable);
writeJson('midway-desktop/packager.beta.generated.json', beta);

const isBeta = channel === 'beta';
const summary = {
  channel,
  repository,
  identifier: isBeta ? beta.identifier : stable.identifier,
  productName: isBeta ? beta.productName : stable.productName,
  homepage,
  publisher,
  // El artefacto de manifiesto del updater que consume este canal (Requisito 8.3):
  // stable -> latest.json, beta -> latest-beta.json.
  updaterManifest: isBeta ? 'latest-beta.json' : 'latest.json',
  configFile: isBeta
    ? 'midway-desktop/packager.beta.generated.json'
    : 'midway-desktop/packager.stable.generated.json'
};
writeJson('out/release-config-summary.json', summary);
console.log(JSON.stringify(summary, null, 2));
