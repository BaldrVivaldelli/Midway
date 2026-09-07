"""Atribuye cada mutación de un campo de `Midway` a la función de producción que la
contiene, recorriendo `midway-desktop/src/app.rs` y `midway-desktop/src/ui/*.rs`.

Una "mutación" es una asignación `state.<campo>… = …` o una llamada a un método mutante
conocido sobre `state.<campo>…`. Las coincidencias dentro de un módulo `#[cfg(test)]` se
excluyen: el módulo de tests se detecta por el atributo y se salta hasta el fin del
archivo o hasta la siguiente declaración de nivel superior fuera del módulo.

Uso (desde la raíz del repo):
    python3 docs/baseline-evidence/state_mutation_map.py
"""

import os
import re
from collections import defaultdict

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

FIELDS = [
    "app_state", "workspace", "tabs", "active_tab", "closed_tabs", "workspace_panel",
    "palette", "runner", "session", "theme_mode", "main_content_focus", "updater",
    "crash_log", "unsaved_changes_prompt", "active_collection_id", "top_bar_mode",
    "tree", "create_collection_prompt", "save_request_prompt", "workspace_crud_dialog",
    "panel_dragging", "panel_hovered",
]

MUTATORS = (
    "push_front|pop_front|pop_back|push|pop|retain|remove|insert|take|iter_mut|get_mut|clear"
)
ASSIGN = re.compile(r"\bstate\.(" + "|".join(FIELDS) + r")\b((?:\.[a-z_]+)*)\s*=[^=]")
CALL = re.compile(r"\bstate\.(" + "|".join(FIELDS) + r")\b((?:\.[a-z_]+)*)\.(" + MUTATORS + r")\(")
# `state.tabs[i].campo = …` y `&mut state.tabs` (préstamo mutable pasado a un helper)
# también son mutaciones, y no las capturan los dos patrones anteriores.
INDEXED = re.compile(r"\bstate\.(" + "|".join(FIELDS) + r")\[[^\]]+\]\s*\.[a-z_]+\s*=[^=]")
BORROW = re.compile(r"&mut\s+state\.(" + "|".join(FIELDS) + r")\b")
INDEX_BORROW = re.compile(r"&mut\s+state\.(" + "|".join(FIELDS) + r")\[")
# Sólo declaraciones de función de nivel superior (sin indentación), que es la forma en
# que están escritos todos los handlers de `app.rs` y de `ui/*.rs`.
FN_DECL = re.compile(r"^(?:pub(?:\([a-z:]+\))?\s+)?(?:async\s+)?fn\s+([a-z_0-9]+)")

TARGETS = ["midway-desktop/src/app.rs"] + [
    os.path.join("midway-desktop/src/ui", name)
    for name in sorted(os.listdir(os.path.join(REPO, "midway-desktop/src/ui")))
    if name.endswith(".rs")
]

hits = defaultdict(set)

def merge_method_chains(raw_lines):
    """Une las continuaciones de cadenas de métodos (`\\n    .iter_mut()`) con su línea
    anterior, para que `state.workspace.collections.iter_mut()` escrito en cuatro líneas
    se detecte igual que si estuviera en una sola. El conteo de llaves no cambia."""
    merged = []
    for line in raw_lines:
        if merged and line.lstrip().startswith("."):
            merged[-1] = merged[-1].rstrip("\n") + line.lstrip()
        else:
            merged.append(line)
    return merged


for rel in TARGETS:
    with open(os.path.join(REPO, rel), encoding="utf-8") as handle:
        lines = merge_method_chains(handle.readlines())

    in_test_mod = False
    test_mod_depth = 0
    depth = 0
    current_fn = "<top-level>"
    pending_test_attr = False

    for line in lines:
        stripped = line.strip()

        if stripped.startswith("#[cfg(test)]"):
            pending_test_attr = True

        decl = FN_DECL.match(stripped)
        if decl and depth == 0:
            current_fn = decl.group(1)

        if not in_test_mod and pending_test_attr and stripped.startswith("mod "):
            in_test_mod = True
            test_mod_depth = depth
            pending_test_attr = False

        is_comment = stripped.startswith("//")

        if not in_test_mod and not is_comment:
            for match in ASSIGN.finditer(line):
                hits[match.group(1)].add((rel, current_fn))
            for pattern in (CALL, INDEXED, BORROW, INDEX_BORROW):
                for match in pattern.finditer(line):
                    hits[match.group(1)].add((rel, current_fn))

        depth += line.count("{") - line.count("}")

        if in_test_mod and depth <= test_mod_depth:
            in_test_mod = False

for field in FIELDS:
    where = sorted(hits.get(field, ()))
    files = sorted({rel for rel, _ in where})
    print(f"{field}: files={files}")
    for rel, fn in where:
        print(f"    {rel}::{fn}")
