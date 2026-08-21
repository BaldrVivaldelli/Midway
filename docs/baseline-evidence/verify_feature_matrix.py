#!/usr/bin/env python3
"""Verifica la forma de `docs/feature-matrix.md` contra el Requisito 4.

Comprobaciones (solo lectura, sin dependencias externas):

1. Cada tabla de la Matriz_Funcionalidades tiene la cabecera exacta del Req 4.3 y
   **exactamente once columnas** en todas sus filas (Req 4.3).
2. Las cinco columnas de estado (UI, dominio, infraestructura, persistencia, tests)
   empiezan con uno de los siete estados cerrados del Req 4.1, o con `No aplica`
   cuando la dimensión no corresponde a la fila.
3. La columna de verificación manual dice `No_Verificable_En_Entorno` en todas las
   filas: no se registra ninguna verificación manual que no se realizó (Req 4.5, 4.8).
4. Ninguna fila está en `Verified` (Req 4.4: no hay evidencia distinta de tests
   unitarios en este entorno).
5. Ninguna fila de la matriz contiene porcentajes de avance (Req 4.2). La lista de
   afirmaciones prohibidas de la sección final cita textualmente al Req 14.5 y por eso
   queda fuera de este chequeo: es una negación, no una medición.

Uso: `python3 docs/baseline-evidence/verify_feature_matrix.py`
Salida 0 = la forma del documento cumple; 1 = hay hallazgos, que se imprimen.
"""

import re
import sys
from pathlib import Path

MATRIX_HEADER = [
    "Funcionalidad",
    "Ubicación en el código",
    "Alcanzable desde la UI",
    "Implementación de dominio",
    "Implementación de infraestructura",
    "Persistencia",
    "Tests",
    "Verificación manual",
    "Limitaciones",
    "Riesgos",
    "Hito objetivo",
]

CLOSED_STATES = (
    "Not started",
    "Scaffolded",
    "Partially wired",
    "Functional",
    "Verified",
    "Blocked",
    "Intentionally unsupported",
)

# Columnas (0-based) que deben empezar con un estado cerrado o con `No aplica`.
STATE_COLUMNS = (2, 3, 4, 5, 6)
MANUAL_COLUMN = 7


def split_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def leading_state(cell: str) -> str | None:
    """Devuelve el estado con que arranca la celda, o None si no arranca con uno."""
    plain = cell.replace("`", "").strip()
    if plain.startswith("No aplica"):
        return "No aplica"
    for state in CLOSED_STATES:
        if plain == state or plain.startswith(state + " ") or plain.startswith(state + " —"):
            return state
    return None


def main() -> int:
    path = Path(__file__).resolve().parents[1] / "feature-matrix.md"
    lines = path.read_text(encoding="utf-8").splitlines()

    failures: list[str] = []
    tables = 0
    rows = 0
    verified_rows: list[str] = []
    percentages: list[str] = []

    for index, line in enumerate(lines):
        if not line.startswith("|"):
            continue
        cells = split_row(line)
        if cells[0] != "Funcionalidad":
            continue
        tables += 1
        if cells != MATRIX_HEADER:
            failures.append(f"L{index + 1}: cabecera distinta de la del Req 4.3: {cells}")
            continue
        cursor = index + 2  # saltar la línea separadora de la tabla
        while cursor < len(lines) and lines[cursor].startswith("|"):
            row = split_row(lines[cursor])
            rows += 1
            if re.search(r"\d+\s?%", lines[cursor]):
                percentages.append(f"L{cursor + 1}: {row[0][:60]}")
            if len(row) != 11:
                failures.append(f"L{cursor + 1}: {len(row)} columnas, se esperaban 11")
                cursor += 1
                continue
            for column in STATE_COLUMNS:
                state = leading_state(row[column])
                if state is None:
                    failures.append(
                        f"L{cursor + 1} col {column + 1}: no arranca con un estado cerrado: {row[column][:70]!r}"
                    )
                elif state == "Verified":
                    verified_rows.append(f"L{cursor + 1} col {column + 1}: {row[0][:50]}")
            manual = row[MANUAL_COLUMN].replace("`", "").split("—")[0].strip()
            if manual != "No_Verificable_En_Entorno":
                failures.append(f"L{cursor + 1}: verificación manual inesperada: {manual!r}")
            cursor += 1

    print(f"tablas de la Matriz_Funcionalidades: {tables}")
    print(f"filas de la Matriz_Funcionalidades: {rows}")
    print(f"filas en `Verified`: {len(verified_rows)}")
    print(f"porcentajes en filas de la matriz: {len(percentages)}")
    for item in percentages:
        print(f"  {item}")
    for item in verified_rows:
        print(f"  Verified sin evidencia distinta de tests unitarios: {item}")

    if verified_rows:
        failures.append("hay filas en `Verified` (Req 4.4: no corresponde en este entorno)")
    if percentages:
        failures.append("hay porcentajes en filas de la matriz (Req 4.2)")

    if failures:
        print("HALLAZGOS:")
        for failure in failures:
            print(f"  {failure}")
        return 1

    print("OK: once columnas exactas, estados cerrados, verificación manual uniforme, sin `Verified`, sin porcentajes.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
