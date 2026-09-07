"""Cuenta ocurrencias de `unwrap()` / `expect(` fuera de los módulos `#[cfg(test)]`.

Método: recorre cada `.rs` de `midway-core/src` y `midway-desktop/src`, detecta cada
atributo `#[cfg(test)]` seguido de una declaración `mod ... {` y saltea el bloque
completo balanceando llaves (ignorando llaves dentro de literales de cadena y de
comentarios de línea de forma aproximada). Lo que queda es código de producción.

El resultado es una cota de la limitación `L21` de `docs/known-limitations.md`, cuyo
recuento (358) es por archivo e incluye los módulos de test alojados en esos archivos.

Uso (desde la raíz del repo):
    python3 docs/baseline-evidence/count_unwrap_prod.py
"""

import pathlib
import re

ROOTS = ["midway-core/src", "midway-desktop/src"]
NEEDLE = re.compile(r"unwrap\(\)|expect\(")


def test_block_ranges(lines):
    """Devuelve la lista de rangos [inicio, fin) de módulos `#[cfg(test)]`."""
    ranges = []
    i = 0
    while i < len(lines):
        if lines[i].strip().startswith("#[cfg(test)]"):
            # Busca la apertura de bloque del `mod` siguiente.
            j = i
            depth = 0
            opened = False
            while j < len(lines):
                depth += lines[j].count("{") - lines[j].count("}")
                if "{" in lines[j]:
                    opened = True
                if opened and depth <= 0:
                    break
                j += 1
            ranges.append((i, min(j + 1, len(lines))))
            i = j + 1
        else:
            i += 1
    return ranges


def production_hits(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    skip = set()
    for start, end in test_block_ranges(lines):
        skip.update(range(start, end))
    return [
        (n + 1, line.strip())
        for n, line in enumerate(lines)
        if n not in skip and NEEDLE.search(line)
    ]


total = 0
per_file = []
for root in ROOTS:
    for path in sorted(pathlib.Path(root).rglob("*.rs")):
        hits = production_hits(path)
        if hits:
            total += len(hits)
            per_file.append((len(hits), str(path), hits))

per_file.sort(reverse=True)
print(f"total fuera de #[cfg(test)]: {total}")
for count, name, hits in per_file:
    print(f"\n{name}: {count}")
    for line_no, text in hits:
        print(f"  {line_no}: {text}")
