import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

// ---------------------------------------------------------------------------
// Generación de SHA256SUMS.txt (Fase 7, Tarea 15.3)
//
// Este script es agnóstico del empaquetador: recorre el directorio de
// artefactos del release (los que produce `cargo-packager`: AppImage/deb en
// Linux, NSIS/MSI en Windows, más `latest.json`/`latest-beta.json`) y emite
// una línea por archivo. No depende de patrones de nombre, por lo que la
// migración Tauri -> cargo-packager no cambia su lógica.
//
// Formato de salida, exactamente el que consume el updater de midway-desktop
// (ver `parse_sha256sums` en midway-desktop/src/updater.rs):
//
//   {hash}  {archivo}\n
//
// es decir: hash SHA256 en hex MINÚSCULAS (`digest('hex')`), DOS espacios de
// separación y el nombre de archivo. El updater busca por ruta exacta y, si no
// la encuentra, cae al basename, de modo que este formato es compatible tanto
// con el directorio plano de `release-assets` como con rutas relativas.
// ---------------------------------------------------------------------------

function walk(dir) {
  const results = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...walk(fullPath));
    } else {
      results.push(fullPath);
    }
  }
  return results;
}

function sha256(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

const [inputDir = 'release-artifacts', outputFile = 'out/SHA256SUMS.txt'] = process.argv.slice(2);

if (!fs.existsSync(inputDir)) {
  console.error(`No existe el directorio de entrada: ${inputDir}`);
  process.exit(1);
}

const files = walk(inputDir)
  .filter((file) => fs.statSync(file).isFile())
  .filter((file) => path.basename(file) !== 'SHA256SUMS.txt')
  .sort((a, b) => a.localeCompare(b));

if (files.length === 0) {
  console.error(`No se encontraron archivos para checksums en ${inputDir}`);
  process.exit(1);
}

const lines = files.map((file) => {
  const hash = sha256(file);
  const relative = path.relative(inputDir, file).replace(/\\/g, '/');
  return `${hash}  ${relative}`;
});

fs.mkdirSync(path.dirname(outputFile), { recursive: true });
fs.writeFileSync(outputFile, `${lines.join('\n')}\n`, 'utf8');
console.log(`Checksums escritos en ${outputFile}`);
