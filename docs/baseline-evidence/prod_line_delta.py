"""Compara el recuento de líneas de producción de dos versiones de un mismo archivo.

Uso:
    python3 docs/baseline-evidence/prod_line_delta.py ANTES.rs DESPUES.rs

"Producción" = todas las líneas del archivo menos los bloques `#[cfg(test)]`
(el mismo criterio de `count_test_lines.py`). El script imprime el recuento total,
el recuento de test, el recuento de producción de cada versión, la diferencia, y el
diff unificado de las líneas de producción para que el delta sea auditable.

Motivo: el recuento crudo de `app.rs` creció respecto de la línea base porque las
tareas 4.1, 4.4, 4.5 y 4.6 agregaron módulos `#[cfg(test)]`. Ese crecimiento no dice
nada sobre la extracción de la vertical. Para aislar el efecto de la extracción hay
que comparar solo código de producción.
"""

import difflib
import sys


def test_spans(lines):
    """Devuelve los rangos (inicio, fin) 0-based inclusive de los bloques #[cfg(test)]."""
    spans = []
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
            spans.append((i, j))
            i = j + 1
        else:
            i += 1
    return spans


def split(path):
    lines = open(path, encoding="utf-8").read().splitlines()
    spans = test_spans(lines)
    in_test = set()
    for start, end in spans:
        in_test.update(range(start, end + 1))
    prod = [line for idx, line in enumerate(lines) if idx not in in_test]
    return lines, prod, spans


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        return 1
    before_path, after_path = sys.argv[1], sys.argv[2]
    before_all, before_prod, before_spans = split(before_path)
    after_all, after_prod, after_spans = split(after_path)

    print(f"antes   {before_path}")
    print(f"  total={len(before_all)} test={len(before_all) - len(before_prod)} "
          f"prod={len(before_prod)} bloques_cfg_test={len(before_spans)}")
    print(f"despues {after_path}")
    print(f"  total={len(after_all)} test={len(after_all) - len(after_prod)} "
          f"prod={len(after_prod)} bloques_cfg_test={len(after_spans)}")
    print()
    print(f"delta total = {len(after_all) - len(before_all):+d}")
    print(f"delta test  = "
          f"{(len(after_all) - len(after_prod)) - (len(before_all) - len(before_prod)):+d}")
    print(f"delta prod  = {len(after_prod) - len(before_prod):+d}")
    print()

    diff = list(difflib.unified_diff(before_prod, after_prod,
                                     fromfile=f"{before_path} (prod)",
                                     tofile=f"{after_path} (prod)",
                                     lineterm=""))
    added = sum(1 for d in diff if d.startswith("+") and not d.startswith("+++"))
    removed = sum(1 for d in diff if d.startswith("-") and not d.startswith("---"))
    print(f"lineas de produccion agregadas = {added}")
    print(f"lineas de produccion removidas = {removed}")
    print()
    print("\n".join(diff))
    return 0


if __name__ == "__main__":
    sys.exit(main())
