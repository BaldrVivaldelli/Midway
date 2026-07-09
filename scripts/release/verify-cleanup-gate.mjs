#!/usr/bin/env node
// Compuerta de bloqueo de la limpieza final (Fase 8) — Tarea 17.4, Requisito 10,
// Criterio 10.8.
//
// El Criterio 10.8 establece: si el reporte de diferencias identifica una
// REGRESIÓN BLOQUEANTE que NO está documentada ni ha sido aceptada
// explícitamente por el responsable técnico, el Workspace_Cargo DEBE bloquear la
// limpieza final (Requisito 9 / Fase 8) hasta que dicha regresión sea resuelta o
// aceptada explícitamente.
//
// Este script convierte esa regla en una compuerta concreta y verificable:
// parsea `docs/differences-report.md`, localiza toda entrada clasificada como
// regresión BLOQUEANTE y, si alguna de ellas NO está marcada como resuelta ni
// aceptada explícitamente, sale con código distinto de cero (bloqueando así la
// Fase 8). Si no hay regresiones bloqueantes abiertas, sale con código 0.
//
// Uso:
//   node scripts/release/verify-cleanup-gate.mjs [ruta-al-reporte]
//
// Por defecto lee docs/differences-report.md relativo a la raíz del repo.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_REPORT = 'docs/differences-report.md';

// Normaliza texto para comparaciones robustas: minúsculas y sin acentos.
function normalize(text) {
  return text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();
}

// Quita el formato markdown inline (negritas/itálicas/código) de una celda.
function stripInlineMarkdown(text) {
  return text.replace(/[*_`]/g, '').trim();
}

// Divide una fila de tabla markdown (`| a | b | c |`) en sus celdas.
function splitRow(line) {
  const trimmed = line.trim().replace(/^\|/, '').replace(/\|$/, '');
  return trimmed.split('|').map((cell) => cell.trim());
}

function isTableRow(line) {
  return line.trim().startsWith('|');
}

// Una fila separadora de encabezado es `| --- | :--: | ... |`.
function isSeparatorRow(line) {
  const cells = splitRow(line);
  return cells.length > 0 && cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

// Detecta si una celda de clasificación indica regresión bloqueante.
// "No bloqueante" (en cualquiera de sus variantes) NO es bloqueante.
function classifyCell(cellText) {
  const value = normalize(stripInlineMarkdown(cellText));
  if (!value) return 'none';
  if (value.includes('no bloqueante')) return 'non-blocking';
  if (value.includes('bloqueante')) return 'blocking';
  return 'other';
}

// Determina si una fila completa está marcada como resuelta o aceptada
// explícitamente por el responsable técnico.
function isResolvedOrAccepted(rowText) {
  const value = normalize(rowText);
  // resuelto/resuelta/resuelt*, aceptad*/aceptacion, resolved, accepted.
  return /(resuelt|aceptad|aceptacion|resolved|accepted)/.test(value);
}

// Detecta filas placeholder/vacías (p. ej. la fila `_(vacío)_` de §5).
function isPlaceholderRow(idCell, cells) {
  const id = normalize(stripInlineMarkdown(idCell));
  if (!id || id === '(vacio)' || id === 'vacio') return true;
  // Fila totalmente vacía salvo separadores.
  return cells.every((cell) => stripInlineMarkdown(cell) === '');
}

/**
 * Parsea el reporte de diferencias y extrae, de toda tabla que tenga una
 * columna "Clasificación", las entradas clasificadas como bloqueantes.
 *
 * @param {string} markdown contenido de differences-report.md
 * @returns {{
 *   entries: Array<{id:string, classification:string, resolvedOrAccepted:boolean, raw:string}>,
 *   openBlockingRegressions: Array<{id:string, raw:string}>
 * }}
 */
export function parseDifferencesReport(markdown) {
  const lines = markdown.split(/\r?\n/);
  const entries = [];

  let i = 0;
  while (i < lines.length) {
    if (!isTableRow(lines[i])) {
      i += 1;
      continue;
    }

    // Comienzo de un bloque de tabla: recolectar filas contiguas.
    const block = [];
    while (i < lines.length && isTableRow(lines[i])) {
      block.push(lines[i]);
      i += 1;
    }

    if (block.length < 2) continue;

    const header = splitRow(block[0]).map((cell) => normalize(stripInlineMarkdown(cell)));
    const classIndex = header.findIndex((cell) => cell.includes('clasificacion'));
    if (classIndex === -1) continue; // No es una tabla de clasificación.

    for (let r = 1; r < block.length; r += 1) {
      const rowLine = block[r];
      if (isSeparatorRow(rowLine)) continue;
      const cells = splitRow(rowLine);
      const idCell = cells[0] ?? '';
      if (isPlaceholderRow(idCell, cells)) continue;

      const classification = classifyCell(cells[classIndex] ?? '');
      if (classification === 'none' || classification === 'other') continue;

      const entry = {
        id: stripInlineMarkdown(idCell) || '(sin id)',
        classification,
        resolvedOrAccepted: isResolvedOrAccepted(rowLine),
        raw: rowLine.trim()
      };
      entries.push(entry);
    }
  }

  const openBlockingRegressions = entries
    .filter((e) => e.classification === 'blocking' && !e.resolvedOrAccepted)
    .map((e) => ({ id: e.id, raw: e.raw }));

  return { entries, openBlockingRegressions };
}

function main(argv) {
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, '..', '..');
  const reportPath = argv[2]
    ? path.resolve(argv[2])
    : path.join(repoRoot, DEFAULT_REPORT);

  if (!fs.existsSync(reportPath)) {
    console.error(`[cleanup-gate] No se encontró el reporte de diferencias: ${reportPath}`);
    console.error('[cleanup-gate] La compuerta BLOQUEA la Fase 8: no hay reporte que verificar (Criterio 10.7/10.8).');
    process.exit(2);
  }

  const markdown = fs.readFileSync(reportPath, 'utf8');
  const { entries, openBlockingRegressions } = parseDifferencesReport(markdown);

  const blockingTotal = entries.filter((e) => e.classification === 'blocking').length;
  const nonBlockingTotal = entries.filter((e) => e.classification === 'non-blocking').length;

  console.log(`[cleanup-gate] Reporte analizado: ${path.relative(repoRoot, reportPath)}`);
  console.log(`[cleanup-gate] Diferencias clasificadas: ${entries.length} (bloqueantes: ${blockingTotal}, no bloqueantes: ${nonBlockingTotal}).`);

  if (openBlockingRegressions.length > 0) {
    console.error('');
    console.error('[cleanup-gate] LIMPIEZA FINAL (FASE 8) BLOQUEADA (Criterio 10.8).');
    console.error('[cleanup-gate] Existen regresiones bloqueantes sin resolver ni aceptar explícitamente:');
    for (const reg of openBlockingRegressions) {
      console.error(`  - ${reg.id}: ${reg.raw}`);
    }
    console.error('');
    console.error('[cleanup-gate] Resuelve o acepta explícitamente cada regresión (marcándola como');
    console.error('               "resuelta"/"aceptada" en docs/differences-report.md) antes de la Fase 8.');
    process.exit(1);
  }

  console.log('[cleanup-gate] No hay regresiones bloqueantes abiertas. Limpieza final (Fase 8) AUTORIZADA por esta compuerta.');
  process.exit(0);
}

// Ejecutar main solo cuando se invoca como script (no al importarlo en tests).
const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
if (invokedPath === fileURLToPath(import.meta.url)) {
  main(process.argv);
}
