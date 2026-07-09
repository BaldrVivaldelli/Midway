#!/usr/bin/env bash
# Verificación de compilación y no-regresión del workspace Cargo (Fase 0+).
#
# Cubre:
#   - Requirement 1.8: `cargo check --workspace` compila sin errores los tres
#     crates miembros (midway-core, midway-desktop, midway/src-tauri).
#   - Requirement 1.9: la suite de tests existente del crate `midway`
#     (src-tauri) sigue pasando sin regresiones tras la extracción de
#     domain/infra/runtime hacia midway-core.
#   - Requirement 10.1-10.3 (checkpoints por fase): la suite de tests de
#     `midway-desktop` (properties, unit e integration tests añadidos desde
#     la Fase 1 en adelante) sigue pasando sin regresiones.
#
# Nota sobre `midway-core`: el test
# `domain::interop::tests::imports_openapi_paths_into_requests` falla de forma
# preexistente (confirmado en la Tarea 1.2 mediante `git stash`, que reprodujo
# el mismo fallo en el código original sin mover). No es una regresión
# introducida por esta migración, por lo que este script lo trata como un
# fallo conocido y NO bloquea la verificación por él — pero SÍ falla si
# aparece cualquier OTRO test fallido en `midway-core`, ya que eso indicaría
# una regresión real introducida por la extracción.
#
# Uso: scripts/verify-workspace.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KNOWN_PREEXISTING_FAILURES=(
  "domain::interop::tests::imports_openapi_paths_into_requests"
)

echo "==> cargo check --workspace"
cargo check --workspace

echo
echo "==> cargo test -p midway (suite existente del crate Tauri, Requirement 1.9)"
cargo test -p midway --no-fail-fast

echo
echo "==> cargo test -p midway-core --no-fail-fast (código extraído en la Tarea 1.2)"
set +e
midway_core_output="$(cargo test -p midway-core --no-fail-fast 2>&1)"
midway_core_status=$?
set -e
echo "$midway_core_output"

if [ "$midway_core_status" -eq 0 ]; then
  echo
  echo "midway-core: todos los tests pasaron."
else
  failed_tests=$(printf '%s\n' "$midway_core_output" | sed -n 's/^test \(.*\) \.\.\. FAILED$/\1/p')

  unexpected_failures=()
  for failed in $failed_tests; do
    is_known=0
    for known in "${KNOWN_PREEXISTING_FAILURES[@]}"; do
      if [ "$failed" = "$known" ]; then
        is_known=1
        break
      fi
    done
    if [ "$is_known" -eq 0 ]; then
      unexpected_failures+=("$failed")
    fi
  done

  if [ "${#unexpected_failures[@]}" -gt 0 ]; then
    echo
    echo "REGRESIÓN DETECTADA: los siguientes tests de midway-core fallaron y NO están" >&2
    echo "en la lista de fallos preexistentes conocidos:" >&2
    for f in "${unexpected_failures[@]}"; do
      echo "  - $f" >&2
    done
    exit 1
  fi

  echo
  echo "midway-core: el único fallo es el preexistente y conocido (no es una regresión):"
  for known in "${KNOWN_PREEXISTING_FAILURES[@]}"; do
    echo "  - $known"
  done
fi

echo
echo "==> cargo test -p midway-desktop --no-fail-fast (properties, unit e integration tests de la Fase 1+)"
cargo test -p midway-desktop --no-fail-fast

echo
echo "Verificación de workspace OK: los tres crates compilan y no hay regresiones."
