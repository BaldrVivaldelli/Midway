"""Atribuye las líneas de producción de `app.rs` a cada grupo del `Message` raíz.

Uso: python3 docs/baseline-evidence/attribute_app_rs.py

La atribución es explícita: cada función de nivel superior de producción se
asigna a un grupo o a `_compartido`. El script falla si alguna función de
producción queda sin clasificar, así que la tabla no puede quedar desalineada
del fuente en silencio.

Respalda las tablas de "líneas de producción atribuidas" del ADR 0004.
"""

import re
import subprocess
import sys

PATH = "midway-desktop/src/app.rs"

GROUPS = {
    "RequestComposer": [
        "update_request_composer",
        "key_value_rows_mut",
        "handle_send_pressed",
        "execute_send",
        "handle_send_completed",
        "evict_inactive_response_bodies",
        "handle_settings_pressed",
        "compute_preview",
        "handle_preview_loaded",
        "handle_url_pasted",
        "close_tab_or_prompt_unsaved_changes",
        "handle_tab_closed",
        "handle_closed_tab_reopened",
        "saved_request_location",
        "open_save_request_prompt",
        "handle_save_request_confirmed",
        "persist_request_from_composer",
        "handle_save_request_completed",
        "handle_unsaved_changes_save_requested",
        "persist_tab_draft",
        "handle_unsaved_changes_save_completed",
        "handle_unsaved_changes_discard_requested",
        "default_tab_for_method",
        "is_draft_empty",
        "tab_is_dirty",
    ],
    "Workspace": [
        "update_workspace",
        "validate_environment_name",
        "handle_environment_submitted",
        "save_environment",
        "handle_environment_saved_result",
        "handle_environment_delete_requested",
        "delete_environment",
        "handle_environment_deleted_result",
        "handle_history_requested",
        "load_workspace_snapshot",
        "handle_workspace_snapshot_loaded",
        "handle_export_submitted",
        "export_workspace_data",
        "handle_export_completed",
        "handle_import_submitted",
        "import_workspace_data",
        "import_http_collection",
        "unique_name_against",
        "handle_import_completed",
    ],
    "WorkspaceCrud": [
        "update_workspace_crud",
        "reconcile_workspace_after_crud",
    ],
    "PanelResize": [
        "update_panel_resize",
        "panel_resize_message_for_event",
        "tree_pane_width_from_cursor",
        "request_panel_width_from_cursor",
        "request_panel_width_for_available",
        "vertical_resize_divider",
    ],
    "ActivityBar": ["update_activity_bar"],
    "Palette": [
        "update_palette",
        "build_palette_items",
        "execute_palette_item",
        "open_saved_request_in_tab",
    ],
    "Keyboard": ["update_keyboard"],
    "Runner": ["update_runner", "collection_runner_progress_stream"],
    "Session": ["update_session", "build_session_snapshot"],
    "TopBar": ["update_top_bar", "enforce_top_bar_mode_after_collection_change"],
    "ResponseInspector": ["update_response_inspector"],
    "Theme": ["update_theme"],
    # Raíz de composición: no pertenece a ningún grupo extraíble.
    "_compartido": [
        "restore_pending_session",
        "resolve_startup_collection",
        "panic_payload_message",
        "guarded_update",
        "guarded_view",
        "update",
        "view",
        "subscription",
        "main_content_pane",
        "debug_content",
        "debug_pane_layout",
        "debug_split_content",
        "test_content",
    ],
}

# Grupos sin función propia: solo una línea de routing en `update`.
ROUTING_ONLY = {"Updater": 1, "Tick": 1}

# Subgrupos de `Workspace` y `RequestComposer`, que el ADR 0004 marca como
# grupos a partir antes de extraer.
SUBGROUPS = {
    "Workspace / environments": [
        "validate_environment_name",
        "handle_environment_submitted",
        "save_environment",
        "handle_environment_saved_result",
        "handle_environment_delete_requested",
        "delete_environment",
        "handle_environment_deleted_result",
    ],
    "Workspace / history": ["handle_history_requested"],
    "Workspace / export-import": [
        "handle_export_submitted",
        "export_workspace_data",
        "handle_export_completed",
        "handle_import_submitted",
        "import_workspace_data",
        "import_http_collection",
        "unique_name_against",
        "handle_import_completed",
    ],
    "Workspace / carga de snapshot": [
        "load_workspace_snapshot",
        "handle_workspace_snapshot_loaded",
    ],
    "Workspace / router": ["update_workspace"],
    "RequestComposer / ciclo de vida de ejecucion": [
        "handle_send_pressed",
        "execute_send",
        "handle_send_completed",
        "evict_inactive_response_bodies",
        "compute_preview",
        "handle_preview_loaded",
    ],
    "RequestComposer / persistencia de request": [
        "saved_request_location",
        "open_save_request_prompt",
        "handle_save_request_confirmed",
        "persist_request_from_composer",
        "handle_save_request_completed",
        "handle_unsaved_changes_save_requested",
        "persist_tab_draft",
        "handle_unsaved_changes_save_completed",
        "handle_unsaved_changes_discard_requested",
    ],
}


def inventory():
    out = subprocess.run(
        [sys.executable, "docs/baseline-evidence/fn_inventory.py", PATH],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    sizes = {}
    for line in out.strip().splitlines():
        name, span, count = line.split("\t")
        sizes[name] = int(count)
    return sizes


def total_and_test():
    out = subprocess.run(
        [sys.executable, "docs/baseline-evidence/count_test_lines.py", PATH],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    m = re.search(r"total=(\d+) test=(\d+) prod=(\d+)", out)
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


sizes = inventory()
total, test, prod = total_and_test()

claimed = set()
for names in GROUPS.values():
    claimed.update(names)
missing = sorted(set(sizes) - claimed)
unknown = sorted(claimed - set(sizes))
if missing:
    sys.exit(f"funciones de produccion sin clasificar: {missing}")
if unknown:
    sys.exit(f"nombres clasificados que no existen en el fuente: {unknown}")

print(f"{PATH}: total={total} test={test} produccion={prod}\n")
print("Grupo\tLineas atribuidas")
attributed = 0
rows = []
for group, names in GROUPS.items():
    if group == "_compartido":
        continue
    n = sum(sizes[x] for x in names)
    attributed += n
    rows.append((group, n))
for group, n in ROUTING_ONLY.items():
    rows.append((group, n))
    attributed += n
rows.append(("Tree", 0))
for group, n in sorted(rows, key=lambda r: -r[1]):
    print(f"{group}\t{n}")

shared = sum(sizes[x] for x in GROUPS["_compartido"])
print(f"\nRaiz de composicion (funciones)\t{shared}")
print(f"Total atribuido a grupos\t{attributed}")
print(f"Produccion no atribuida (raiz + enums + structs + imports)\t{prod - attributed}")

print("\nSubgrupo\tLineas atribuidas")
for name, fns in SUBGROUPS.items():
    print(f"{name}\t{sum(sizes[x] for x in fns)}")
