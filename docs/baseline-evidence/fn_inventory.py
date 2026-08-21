"""Inventario de funciones de nivel superior de un archivo Rust, con rango de líneas.

Uso: python3 docs/baseline-evidence/fn_inventory.py <archivo.rs>

Solo considera funciones de nivel superior (columna 0) fuera de bloques
`#[cfg(test)]`. Cuenta desde la firma hasta la llave de cierre del cuerpo por
balance de llaves. Respalda las tablas de "líneas de producción atribuidas"
del ADR 0004.
"""

import re
import sys

path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()

# Rangos de bloques #[cfg(test)] a excluir.
test_spans = []
i = 0
while i < len(lines):
    if lines[i].strip() == "#[cfg(test)]":
        depth = 0
        opened = False
        j = i
        while j < len(lines):
            depth += lines[j].count("{") - lines[j].count("}")
            if "{" in lines[j]:
                opened = True
            if opened and depth <= 0:
                break
            j += 1
        test_spans.append((i, j))
        i = j + 1
    else:
        i += 1


def in_test(idx):
    return any(a <= idx <= b for a, b in test_spans)


pat = re.compile(r"^(pub )?(async )?fn ([A-Za-z0-9_]+)")
for i, line in enumerate(lines):
    m = pat.match(line)
    if not m or in_test(i):
        continue
    depth = 0
    opened = False
    j = i
    while j < len(lines):
        depth += lines[j].count("{") - lines[j].count("}")
        if "{" in lines[j]:
            opened = True
        if opened and depth <= 0:
            break
        j += 1
    print(f"{m.group(3)}\t{i + 1}-{j + 1}\t{j - i + 1}")
