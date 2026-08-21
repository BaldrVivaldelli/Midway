"""Mide el rango de líneas de funciones nombradas dentro de un archivo Rust.

Uso: python3 docs/baseline-evidence/fn_span.py <archivo.rs> <fn> [<fn> ...]

Cuenta desde la línea de la firma hasta la llave de cierre del cuerpo,
usando balance de llaves. Es la medición que respalda las tablas de
"líneas de producción atribuidas" del ADR 0004.
"""

import re
import sys

path = sys.argv[1]
names = sys.argv[2:]
lines = open(path, encoding="utf-8").read().splitlines()

for name in names:
    pat = re.compile(r"^(pub )?(async )?fn " + re.escape(name) + r"\b")
    found = False
    for i, line in enumerate(lines):
        if pat.match(line):
            found = True
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
            print(f"{name}: {i + 1}-{j + 1} = {j - i + 1} lineas")
    if not found:
        print(f"{name}: no hallada en {path}")
