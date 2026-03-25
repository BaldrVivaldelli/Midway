import fs from 'node:fs';
import path from 'node:path';

const distDir = path.resolve('dist/assets');
const MAX_JS_CHUNK_BUDGET = 700 * 1024;
const JS_TOTAL_BUDGET = 760 * 1024;
const CSS_BUDGET = 32 * 1024;
const TOTAL_BUDGET = 800 * 1024;

function size(filePath) {
  return fs.statSync(filePath).size;
}

function format(bytes) {
  return `${(bytes / 1024).toFixed(1)} KB`;
}

if (!fs.existsSync(distDir)) {
  console.error('No existe dist/assets. Corré primero npm run build.');
  process.exit(1);
}

const files = fs.readdirSync(distDir).map((name) => path.join(distDir, name));
const jsFiles = files.filter((file) => file.endsWith('.js'));
const cssFiles = files.filter((file) => file.endsWith('.css'));

const jsTotal = jsFiles.reduce((acc, file) => acc + size(file), 0);
const cssTotal = cssFiles.reduce((acc, file) => acc + size(file), 0);
const total = files.reduce((acc, file) => acc + size(file), 0);
const largestJs = jsFiles
  .map((file) => ({ file: path.basename(file), bytes: size(file) }))
  .sort((left, right) => right.bytes - left.bytes)[0] ?? null;

const violations = [];
if (largestJs && largestJs.bytes > MAX_JS_CHUNK_BUDGET) {
  violations.push(
    `Chunk principal ${largestJs.file} ${format(largestJs.bytes)} > ${format(MAX_JS_CHUNK_BUDGET)}`
  );
}
if (jsTotal > JS_TOTAL_BUDGET) violations.push(`JS total ${format(jsTotal)} > ${format(JS_TOTAL_BUDGET)}`);
if (cssTotal > CSS_BUDGET) violations.push(`CSS ${format(cssTotal)} > ${format(CSS_BUDGET)}`);
if (total > TOTAL_BUDGET) violations.push(`Total ${format(total)} > ${format(TOTAL_BUDGET)}`);

if (violations.length > 0) {
  console.error('Size budget failed:');
  for (const violation of violations) {
    console.error(`- ${violation}`);
  }
  console.error('\nTop JS chunks:');
  for (const entry of jsFiles
    .map((file) => ({ file: path.basename(file), bytes: size(file) }))
    .sort((left, right) => right.bytes - left.bytes)
    .slice(0, 5)) {
    console.error(`- ${entry.file}: ${format(entry.bytes)}`);
  }
  process.exit(1);
}

console.log(
  `Budgets OK · chunk principal ${largestJs ? `${largestJs.file} ${format(largestJs.bytes)}` : 'n/a'} · JS ${format(jsTotal)} · CSS ${format(cssTotal)} · total ${format(total)}`
);
