import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

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
