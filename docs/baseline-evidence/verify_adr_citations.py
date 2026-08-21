"""Verifica las citas `archivo:linea` de los ADRs contra el fuente.

Uso: python3 docs/baseline-evidence/verify_adr_citations.py

Cada entrada de CHECKS es (archivo, linea, subcadena esperada en esa linea).
Sale con codigo != 0 si alguna cita no coincide.
"""

import sys

APP = "midway-desktop/src/app.rs"
TB = "midway-desktop/src/ui/top_bar.rs"
DS = "midway-desktop/src/ui/design_system.rs"
RI = "midway-desktop/src/ui/response_inspector.rs"
MAIN = "midway-desktop/src/main.rs"
SESSION = "midway-desktop/src/session.rs"
TREE = "midway-desktop/src/ui/request_tree_pane.rs"

CHECKS = [
    # ADR 0003
    (APP, 58, "pub enum Message {"),
    (APP, 88, "}"),
    (APP, 565, "pub enum ThemeMessage {"),
    (APP, 569, "}"),
    (APP, 2123, "fn update_theme"),
    (APP, 2131, "}"),
    (APP, 1380, "pub theme_mode: ThemeMode,"),
    (APP, 1302, "pub theme_mode: ThemeMode,"),
    (APP, 1442, "let theme_mode = session.theme_mode;"),
    (APP, 1539, "session.theme_mode = snapshot.theme_mode;"),
    (APP, 2899, "theme_mode: state.theme_mode,"),
    (APP, 4319, "fn update_response_inspector"),
    (APP, 4330, "}"),
    (APP, 1815, "Message::Updater(_) => Task::none()"),
    (APP, 5560, "DesignSystem::for_mode(state.theme_mode)"),
    (TB, 144, "let theme_icon = match state.theme_mode {"),
    (TB, 160, "on_press(Message::Theme(ThemeMessage::Toggled))"),
    (DS, 135, "pub fn for_mode(mode: ThemeMode) -> Self {"),
    (RI, 48, "DesignSystem::for_mode(state.theme_mode)"),
    (RI, 61, "DesignSystem::for_mode(state.theme_mode)"),
    (MAIN, 32, "DesignSystem::for_mode(state.theme_mode)"),
    (MAIN, 44, "match state.theme_mode {"),
    (SESSION, 124, "pub theme_mode: ThemeMode,"),
    # ADR 0004
    (APP, 1802, "Message::Tick => Task::none()"),
    (APP, 103, "pub enum PanelResizeMessage {"),
    (APP, 116, "}"),
    (APP, 128, "pub enum RequestComposerMessage {"),
    (APP, 394, "}"),
    (APP, 397, "pub enum ResponseInspectorMessage {"),
    (APP, 401, "}"),
    (APP, 404, "pub enum WorkspaceMessage {"),
    (APP, 493, "}"),
    (APP, 502, "pub enum RunnerMessage {"),
    (APP, 532, "}"),
    (APP, 540, "pub enum PaletteMessage {"),
    (APP, 562, "}"),
    (APP, 572, "pub enum SessionMessage {"),
    (APP, 588, "}"),
    (APP, 591, "pub enum UpdaterMessage {"),
    (APP, 596, "}"),
    (APP, 620, "pub enum KeyboardMessage {"),
    (APP, 669, "}"),
    (APP, 829, "pub enum WorkspaceCrudMessage {"),
    (APP, 842, "}"),
    (APP, 846, "pub enum ActivityBarMessage {"),
    (APP, 865, "}"),
    (APP, 869, "pub enum TreeMessage {"),
    (APP, 900, "}"),
    (APP, 904, "pub enum TopBarMessage {"),
    (APP, 913, "}"),
    (APP, 161, "EnvironmentChanged(Option<String>)"),
    (TREE, 226, "pub fn update_tree"),
    (TREE, 408, "}"),
    # Modulos de test citados por el ADR 0004
    (APP, 2676, "mod panel_resize_tests {"),
    (APP, 5180, "mod import_name_collision_tests {"),
    (APP, 5469, "mod debug_layout_tests {"),
    (APP, 5820, "mod tests {"),
    (APP, 9979, "mod error_boundary_tests {"),
    (APP, 10250, "mod resolve_startup_collection_tests {"),
    (APP, 10353, "mod navigation_toggle_preserves_composer_state_tests {"),
    (APP, 10579, "mod collection_selection_workspace_property_tests {"),
]

cache = {}
failures = []
for path, lineno, expected in CHECKS:
    if path not in cache:
        cache[path] = open(path, encoding="utf-8").read().splitlines()
    lines = cache[path]
    if lineno > len(lines):
        failures.append(f"{path}:{lineno} fuera de rango (archivo de {len(lines)} lineas)")
        continue
    actual = lines[lineno - 1]
    if expected not in actual:
        failures.append(f"{path}:{lineno} esperaba {expected!r}, encontro {actual.strip()!r}")

print(f"citas verificadas: {len(CHECKS)}")
if failures:
    print(f"fallos: {len(failures)}")
    for f in failures:
        print(f"  {f}")
    sys.exit(1)
print("todas las citas coinciden con el fuente")
