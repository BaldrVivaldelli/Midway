# Limitaciones conocidas y alcance diferido — Midway

> Registro de lo que hoy **no** funciona, no está alcanzable o no está verificado en la
> rama `main` (HEAD `1fc270326e5c304f24ce08fe1f528da33a007d3e`), y de lo que queda
> explícitamente fuera del alcance de este spec con un hito propietario nombrado.
>
> Cada limitación se derivó leyendo el fuente o la evidencia de
> `docs/baseline-evidence/`, no de lo que declara el README. Cuando la evidencia es una
> corrida de comando, se cita la sección de `docs/product-audit.md` que la transcribe.
>
> Estructura: dos tablas de contenido —limitaciones conocidas (§2) y alcance diferido
> (§3)—, precedidas por el registro de hitos (§1) que existe solo para que ningún elemento
> quede sin dueño nombrado (Req 12.5).
>
> Alcance de este archivo al cierre de la tarea 2.3: las tres secciones quedan escritas.
> Las tareas posteriores del spec **agregan filas a la tabla de la §2**, no reescriben el
> documento: la tarea 4.8 agrega los fallos de Test_Caracterización sobre el baseline sin
> cambios (Req 6.7) y la tarea 11.3 agrega la excepción de idioma de los identificadores de
> modo `Debug` y `Test` que queden sin unificar (Req 9.6). Los identificadores `L…` de la
> columna `Limitación` son estables para que esas tareas puedan referenciarlos y continuar
> la numeración desde `L35`. Las notas de la §4 dicen qué falta registrar y quién lo
> registra.
>
> Actualización de la tarea 11.3: las excepciones de idioma que quedan en pie después de
> unificar el explorador, el editor de request y el inspector de respuesta están registradas
> como `L39` a `L43` (Req 9.6). Las dos entradas previstas en la §4 quedan entonces
> cerradas. `L31` se preserva como registro del baseline; la §4 detalla, grupo por grupo,
> cuánto de lo que enumera sigue vigente y cuánto quedó resuelto.
>
> Actualización de la tarea 4.8: ningún Test_Caracterización falla sobre el baseline sin
> cambios, así que no hay defectos por fallo de test. Lo que sí apareció al fijar los
> límites vigentes son **cuatro comportamientos del baseline previamente no documentados**,
> registrados como `L35` a `L38` (Req 6.6, 6.7). Cada uno lleva hito propietario y criterio
> de cierre en la misma celda, sin agregar columnas a la tabla. La corrida completa que lo
> sustenta está en `docs/product-audit.md` §3.14.

## 1. Registro de hitos

Los hitos 0 y 1 son los de este spec. Los demás son **nombres de dueño**, no diseño: se
declaran acá únicamente para que ninguna limitación ni ningún elemento diferido quede
huérfano (Req 12.5). Ninguno se diseña en este documento (Req 12.6).

| Hito | Alcance en una línea |
| --- | --- |
| Hito 0 | Auditoría verificable de `main`: evidencia, matriz, limitaciones, mapas, ADRs y tests de caracterización (este spec) |
| Hito 1 | Primera vertical extraída de `app.rs`, primer incremento de UX y refuerzo de CI (este spec) |
| Hito 2 | Descomposición de `app.rs` por grupos de mensajes y limpieza de los archivos sobre 1 000 líneas |
| Hito 3 | Herramientas y reproducibilidad de build: `xtask`, dependencias de sistema documentadas, matriz multiplataforma, docs y auditoría de dependencias en CI |
| Hito 4 | Cableado de funcionalidad ya implementada pero inalcanzable desde la UI, y robustez de arranque |
| Hito 5 | Rediseño de la representación de respuesta a bytes |
| Hito 6 | Re-arquitectura de persistencia: esquema versionado, respaldo, migración transaccional y rollback |
| Hito 7 | Adaptadores multiprotocolo |
| Hito 8 | Superficie de colaboración: mocks, monitores, flows y proxy de captura |
| Hito 9 | Extensibilidad: plugins WASM, requests asistidos por IA y scripting |
| Hito 10 | Transporte avanzado, autenticación y endurecimiento del manejo de secretos |

## 2. Limitaciones conocidas

| Limitación | Ubicación en el código | Hito propietario |
| --- | --- | --- |
| L1 — El baseline **no compila en Linux** sin las bibliotecas de desarrollo `libdbus-1` y `libsqlite3`, y esa dependencia de sistema no está documentada: el build script de `libdbus-sys` entra en `explicit panic` cuando `pkg-config` no encuentra `dbus-1.pc`, y con `dbus-1` resuelto los targets de test siguen fallando al enlazar con `cannot find -lsqlite3`. Ni el README ni el `Makefile` enumeran requisitos de sistema. | Cadenas de dependencia declaradas en `midway-core/Cargo.toml`: `midway-core` → `keyring 3` con feature `sync-secret-service` → `dbus-secret-service` → `dbus` → `libdbus-sys` (build script vía `pkg-config`), y `midway-core` → `tokio-rusqlite` → `rusqlite` → `libsqlite3-sys` (enlace contra `-lsqlite3`); ambas cadenas verificadas en `Cargo.lock`. Omisión en `README.md` §"Cómo levantar el proyecto" y en el `Makefile` (targets `run`, `build`, `check`, `test`, `verify`, sin requisitos previos). Evidencia: `docs/product-audit.md` §3.2, §3.6, §3.7 y §4; `docs/baseline-evidence/02-cargo-check.txt`, `04b-cargo-test-with-pkgconfig.txt` | Hito 3 |
| L2 — **Resuelta en la tarea 13.2.** El gate local declaraba menos de lo que exige el proyecto: `verify` corría solo `check` y `test`, con `fmt-check` y `clippy` fuera del gate. Ahora `verify: fmt-check check clippy test` declara la misma intención que CI. En este entorno `make verify` falla en `fmt-check` por `Sin_Herramienta` (rustfmt ausente), no por código; ese resto es `L4`. | `Makefile`, target `verify: fmt-check check clippy test`. Evidencia: `docs/product-audit.md` §3.18 | Hito 1 (tarea 13.2), cerrada |
| L3 — CI no verifica formato ni lints: el único job corre `cargo check --workspace` y `cargo test --workspace`. | `.github/workflows/ci.yml`, job `workspace` | Hito 1 (tarea 13.1) |
| L4 — En el entorno de desarrollo verificado, `cargo fmt`, `cargo clippy`, `cargo deny`, `cargo audit` y `cargo llvm-cov` no están instalados (rustc 1.95.0 desde tarball, sin `rustup`): el estado de formato, lints, licencias, vulnerabilidades y cobertura es **desconocido**, no "bueno". Reconfirmado en la tarea 4.8: `cargo fmt` y `cargo clippy` siguen en `Sin_Herramienta` con exit 101, así que tampoco hay evidencia de formato ni de lints para los tests agregados por las tareas 4.1 a 4.7. | Entorno de ejecución, no código. Evidencia: `docs/product-audit.md` §2, §3.1, §3.5, §3.11–§3.13 y §3.15 | Hito 3 |
| L5 — CI corre en una sola plataforma (`ubuntu-22.04`), sin matriz multiplataforma, sin generación de docs y sin auditoría de dependencias. El pipeline de release empaqueta Windows y Linux x86_64, **sin macOS**, aunque macOS figura como plataforma de primer nivel. | `.github/workflows/ci.yml`; `.github/workflows/release.yml`, job `build` (matriz `ubuntu-22.04` + `windows-latest`) | Hito 3 |
| L6 — CI instala `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev` y `patchelf` sin evidencia completa de que sigan siendo necesarias tras retirar Tauri. **Sigue abierta con evidencia parcial** (tarea 14): con las cuatro ausentes a la vez en el entorno de auditoría, `cargo check --workspace` y `cargo test --workspace` dan exit 0 con 363 tests, y ningún crate de la pila WebKit/GTK/Tauri está en el grafo de 543 paquetes. Falta lo que este entorno no puede correr: la corrida por dependencia en `ubuntu-22.04` (requiere commit y push, prohibidos por el Req 2.1-2.3) y `cargo packager`. Las cuatro **permanecen instaladas**; tres quedan con propuesta de eliminación y `patchelf` se conserva por una presunción de empaquetado AppImage no verificada. | `.github/workflows/ci.yml`, paso "Install Linux system dependencies" (también en `release.yml`, tres jobs). Evidencia: `docs/product-audit.md` §14.1; `docs/baseline-evidence/09-system-dep-probe.txt`, `09b-system-dep-findings.md` | Hito 1 (tarea 14) |
| L7 — No existe el crate `xtask` ni el directorio `scripts/`: los pasos de build, empaquetado y release viven repartidos entre el `Makefile`, `[package.metadata.packager]` y los workflows. | `Cargo.toml` (`members = ["midway-core", "midway-desktop"]`); `Makefile`; `midway-desktop/Cargo.toml` | Hito 3 |
| L8 — `midway-desktop/src/app.rs` concentra 10 750 líneas: el `Message` raíz, el `update` completo, el estado de la app y buena parte de la vista. | `midway-desktop/src/app.rs` | Hito 2 (reducción parcial en el Hito 1) |
| L9 — Seis archivos más superan las 1 000 líneas: `sqlite_repository.rs` 2 322, `updater.rs` 2 311, `request_tree_pane.rs` 2 115, `interop.rs` 2 068, `collection_runner.rs` 1 497, `session.rs` 1 054. | `midway-core/src/infra/sqlite_repository.rs`, `midway-desktop/src/updater.rs`, `midway-desktop/src/ui/request_tree_pane.rs`, `midway-core/src/domain/interop.rs`, `midway-desktop/src/collection_runner.rs`, `midway-desktop/src/session.rs` | Hito 2 |
| L10 — El flujo del updater in-app está implementado pero **nunca se ejecuta**: `Message::Updater(_)` devuelve `Task::none()`, el módulo entero lleva `#![allow(dead_code)]` y `check_for_update` no tiene ningún llamador de producción. El README lo presenta como funcionalidad disponible. | `midway-desktop/src/app.rs` (`Message::Updater(_) => Task::none()`); `midway-desktop/src/updater.rs` (`#![allow(dead_code)]`, `pub async fn check_for_update`) | Hito 4 |
| L11 — `Message::Tick` es una variante inerte: su handler devuelve `Task::none()`. El autosave real viaja por `SessionMessage::AutosaveTick` desde `iced::time::every`. | `midway-desktop/src/app.rs` (`Message::Tick => Task::none()`, `#[allow(dead_code)]`; `subscription`) | Hito 4 |
| L12 — El Collection_Runner no es disparable ni cancelable desde la UI: `RunnerMessage::StartRequested` y `CancelRequested` están marcados `#[allow(dead_code)]` y no existe ningún botón "Run" ni "Cancelar". La vista del modo Test es un placeholder que dice "Presioná Run" sin que ese control exista. | `midway-desktop/src/app.rs` (`RunnerMessage`, `fn test_content`) | Hito 4 |
| L13 — No hay tira de tabs de request: `RequestComposerMessage::TabClosed` y `TabSelected` están marcados `#[allow(dead_code)]`; abrir, cambiar y cerrar tabs solo llega por atajos (`Ctrl+W`, `Alt+1..9`, `Ctrl+Shift+T`) o por el árbol y el palette. | `midway-desktop/src/app.rs` (`RequestComposerMessage`, `fn debug_content`) | Hito 4 |
| L14 — Dos secciones del Workspace_Panel son inalcanzables: `workspace_panel::view` es código muerto y es el único emisor de `WorkspaceMessage::SectionSelected`, así que solo se llega a `Environments` (por default), `History` (icono Activity) y `Data` (al elegir una colección en el palette). `Diagnostics` y `App updates` no tienen camino de navegación. | `midway-desktop/src/ui/workspace_panel.rs` (`#[allow(dead_code)] pub fn view`, `fn section_tabs`); `midway-desktop/src/app.rs` (`update_activity_bar`, `execute_palette_item`) | Hito 4 |
| L15 — El atajo `Ctrl+.` alterna `workspace_panel.collapsed`, pero ninguna vista alcanzable lee ese campo (solo lo lee `workspace_panel::view`, que es código muerto): el atajo no produce efecto observable. | `midway-desktop/src/app.rs` (`WorkspaceMessage::ToggleCollapsed`, `global_shortcut_subscription`); `midway-desktop/src/ui/workspace_panel.rs` | Hito 4 |
| L16 — No hay gestión de secretos desde la UI: `SecretExecutorHandle::set`/`delete` y `SqliteRepository::upsert_secret_metadata`/`delete_secret_metadata` no tienen llamador en `midway-desktop`; solo se leen valores por alias al resolver `{{secret:...}}`. | `midway-core/src/runtime/secret_executor.rs`; `midway-core/src/infra/sqlite_repository.rs`; `midway-desktop/src/app.rs` (`execute_send`, `compute_preview`) | Hito 4 |
| L17 — La cancelación manual de un request en curso no está cableada: `RequestExecutorHandle::cancel` existe en `midway-core` y no tiene ningún llamador en `midway-desktop`. El README la enumera como funcionalidad de esta versión. | `midway-core/src/runtime/request_executor.rs` (`pub async fn cancel`); `midway-desktop/src/` (sin llamador) | Hito 4 |
| L18 — Import y export operan sobre una ruta escrita a mano en un `text_input` ("Ruta de destino (ej. /home/user/export.json)"), sin selector de archivos del sistema. | `midway-desktop/src/ui/workspace_panel.rs` (`export_section`, `import_section`) | Hito 4 |
| L19 — Dos utilidades públicas quedaron sin llamador y anotadas como tal: `diagnostics::clear_crash_records` (la acción "limpiar diagnósticos" no tiene botón) y `session::read_session_snapshot`. | `midway-desktop/src/diagnostics.rs`; `midway-desktop/src/session.rs` | Hito 4 |
| L20 — Si `AppState::initialize` falla (directorio de datos no resoluble o SQLite no abrible), el arranque entra en pánico por un `.expect(...)`: no hay mensaje de error en la UI ni degradación. | `midway-desktop/src/main.rs` (`fn boot`) | Hito 4 |
| L21 — `unwrap()`/`expect()` sin justificación escrita: 358 ocurrencias en `midway-core/src` y `midway-desktop/src`, concentradas en `sqlite_repository.rs` (97), `app.rs` (82), `updater.rs` (48), `session.rs` (32) y `collection_runner.rs` (31). El conteo es por archivo e **incluye los módulos `#[cfg(test)]` alojados en esos mismos archivos**: no se afirma que las 358 sean de producción. | `midway-core/src/**`, `midway-desktop/src/**` | Hito 2 |
| L22 — El texto es la representación canónica de la respuesta, no los bytes: `ResponseEnvelope.body_text: String` y `http_reqwest.rs` cae en `String::from_utf8_lossy` cuando el payload no es UTF-8 válido, con pérdida irreversible en respuestas binarias. `size_bytes` mide el texto retenido, no el payload original. | `midway-core/src/domain/http.rs` (`ResponseEnvelope`); `midway-core/src/infra/http_reqwest.rs` | Hito 5 |
| L23 — El body se recorta a `DEFAULT_MAX_BODY_BYTES` = 8 MiB (flag `truncated`) y la app libera el body de las tabs inactivas (flag `body_evicted`, `body_text` vaciado): el payload completo no queda disponible ni recuperable. | `midway-core/src/domain/http.rs` (`DEFAULT_MAX_BODY_BYTES`); `midway-core/src/infra/http_reqwest.rs`; `midway-desktop/src/app.rs` (eviction) | Hito 5 |
| L24 — La superficie de dominio es más angosta que la esperable en un cliente API: `HttpMethod::ALL` tiene 7 variantes (sin `CONNECT`, `TRACE` ni método personalizado), `BodyMode` 4 (`None`/`Json`/`Text`/`FormData`) y `AuthConfig` 4 (`None`/`Bearer`/`Basic`/`ApiKey`, sin OAuth2). La decisión `Not started` vs `Intentionally unsupported` por variante vive en el ADR 0007. | `midway-core/src/domain/http.rs` | Hito 10 |
| L25 — La persistencia SQLite no tiene versión de esquema: `migrate()` es un `CREATE TABLE IF NOT EXISTS` más un `ALTER TABLE` condicional por `PRAGMA table_info`, sin `user_version`, sin respaldo previo, sin migración transaccional, sin validación posterior y sin rollback. | `midway-core/src/infra/sqlite_repository.rs` (`pub async fn migrate`) | Hito 6 |
| L26 — La sesión no migra: cualquier `session.json` corrupto o con `version` distinta de `SESSION_SCHEMA_VERSION` se descarta por completo, y el usuario pierde tabs abiertas, tab activa, tamaños de panel y stack de tabs cerradas. | `midway-desktop/src/session.rs` (`SESSION_SCHEMA_VERSION`, `load_session_or_default_at`) | Hito 6 |
| L27 — Riesgo de fuga de secretos en el historial: el envío resuelve el request con `SecretRenderMode::Resolve` y persiste la URL resuelta en `history.url`. Si un `{{secret:...}}` se interpola en la URL o en el query string, su valor queda en claro en `workspace.sqlite3`, se muestra en la sección History y viaja en el export nativo cuando incluye historial. El preview sí usa `Redact`. | `midway-desktop/src/app.rs` (`execute_send`); `midway-core/src/infra/sqlite_repository.rs` (`append_history`); `midway-core/src/domain/interop.rs` (`make_native_bundle`) | Hito 10 |
| L28 — El cliente HTTP no soporta proxy, mTLS ni CA propia: se construye con redirecciones limitadas y cookie jar, sin ninguna de esas opciones. | `midway-desktop/src/state.rs` (`AppState::initialize`) | Hito 10 |
| L29 — En el layout apilado del área Debug, el "divisor" del panel de respuesta es un `container` inerte de 1 px y la altura sale de un `.clamp(180.0, 360.0)` escrito en línea: el panel de respuesta no se puede redimensionar por arrastre. | `midway-desktop/src/app.rs` (`debug_split_content`, rama `DebugPaneLayout::Stacked`) | Hito 1 (tarea 10) |
| L30 — Cuatro estados vacíos informan la ausencia sin ofrecer la acción siguiente: "Sin respuesta aún", "No hay cookies almacenadas", "Sin assertions configuradas" y "Midway Desktop — no hay tabs abiertas.". | `midway-desktop/src/ui/response_inspector.rs`; `midway-desktop/src/app.rs` (`debug_content`) | Hito 1 (tarea 11.2) |
| L31 — El texto visible mezcla idiomas: junto a las cadenas en español conviven "Send"/"Sending...", las etiquetas de sección "Environments"/"Data"/"History"/"Diagnostics"/"App updates", los identificadores de modo "Debug"/"Test" y el encabezado "Collection Runner". | `midway-desktop/src/ui/request_composer.rs`; `midway-desktop/src/ui/top_bar.rs` (`section_display_label`, `mode_tab`); `midway-desktop/src/ui/workspace_panel.rs` (`section_tabs`); `midway-desktop/src/app.rs` (`test_content`) | Hito 1 (tarea 11.3) |
| L32 — Las tabs de sección del Workspace_Panel derivan su paleta de `DesignSystem::for_mode(ThemeMode::default())` en lugar del tema activo, y `section_content` recibe el `DesignSystem` y lo ignora (`_ds`): el tema claro no se aplica de forma consistente en ese panel. | `midway-desktop/src/ui/workspace_panel.rs` (`section_tabs`, `section_content`) | Hito 2 |
| L33 — Deuda de proceso en comentarios y documentación: el fuente referencia números de fase y tarea obsoletos ("Fase 6", "Fase 8", "Tarea 11.14"), el encabezado de `ci.yml` afirma "100% Rust" sin evidencia registrada, y tanto el README como varios comentarios de `ui/` citan `iced_aw` como dependencia de tabs cuando `iced_aw` no está en ningún `Cargo.toml` ni en `Cargo.lock`: `ui/tab_bar.rs` es una implementación propia. | `.github/workflows/ci.yml` (comentario de cabecera); `README.md` ("Stack técnico"); `midway-desktop/src/ui/tab_bar.rs`, `request_composer.rs`, `response_inspector.rs`, `workspace_panel.rs` | Hito 3 (encabezado de `ci.yml`: Hito 1, tarea 13.1) |
| L34 — Ninguna funcionalidad gráfica está verificada manualmente en el entorno de auditoría: `DISPLAY` no está definida y solo hay `WAYLAND_DISPLAY=wayland-1`, así que toda verificación con interfaz queda como `No_Verificable_En_Entorno` en `docs/feature-matrix.md`. Los 301 tests que pasan son lógica pura y de dominio; ninguno abre una ventana ni valida render. Sigue valiendo tras la tarea 4.8, con 323 tests: los 22 Test_Caracterización agregados por las tareas 4.1 a 4.7 también son headless. | Entorno de ejecución, no código. Evidencia: `docs/product-audit.md` §2, §3.8 y §3.14 | Hito 3 |
| L35 — **`NaN` no se acota, se propaga.** `f32::clamp` deja pasar `NaN`, así que `tree_pane_width_from_cursor(NaN)` y `request_panel_width_from_cursor(NaN, _)` devuelven `NaN`. No hay pánico —`clamp` solo entra en pánico si `min > max`—, pero el contrato "el resultado queda clampeado" **no se cumple para `NaN`**. La asimetría también está fijada: `request_panel_width_for_available(400.0, NaN)` devuelve el mínimo porque `f32::max` descarta `NaN` antes de que `clamp` lo vea, mientras que un ancho **almacenado** `NaN` sí se propaga. Comportamiento previamente desconocido, detectado al fijar los límites en la tarea 4.6; ningún test falla. | `midway-desktop/src/app.rs`: `tree_pane_width_from_cursor` (2660-2662), `request_panel_width_from_cursor` (2664-2667), `request_panel_width_for_available` (2669-2673). Fijado por `panel_resize_tests::tree_pane_width_handles_degenerate_cursor_coordinates`, `request_panel_width_from_cursor_handles_degenerate_inputs` y `request_panel_width_for_available_handles_non_finite_inputs` | Hito 1 (tarea 10). **Criterio de cierre:** las funciones nuevas de altura de la tarea 10.1 (`response_panel_height_from_cursor`, `response_panel_height_for_available`) quedan clampeadas y sin pánico ante valores no finitos, y la Property 5 de la tarea 10.4 cubre las cinco funciones con entradas que incluyen `NaN`, `+inf` y `-inf`. Si la Property 5 obliga a sanear `NaN` en las tres funciones de ancho vigentes, ese cambio se decide con el usuario: no entra en esta tarea |
| L36 — **Por debajo de 451 px de ancho disponible el panel derecho recibe menos que su propio mínimo.** `max_width` se satura en `DEBUG_PANE_MIN_WIDTH`, así que el editor de request retiene 220 px y al panel de respuesta le queda `available - 10 - 220`, que llega a cero o a negativo. 450 px todavía da 220; **451 es el primer valor que deja crecer al editor** (devuelve 221). El breakpoint a layout apilado está en 560 px, por lo que la franja afectada solo se alcanza con la ventana por debajo de ese umbral. Comportamiento previamente desconocido. | `midway-desktop/src/app.rs`: `request_panel_width_for_available` (2669-2673), con `DEBUG_DIVIDER_HIT_WIDTH = 10.0` (2654) y `DEBUG_PANE_MIN_WIDTH = 220.0` (2655). Fijado por `panel_resize_tests::request_panel_width_for_available_collapses_to_minimum_when_space_is_tight` | Hito 1 (tarea 10). **Criterio de cierre:** el reparto del área Debug garantiza el mínimo de **ambos** paneles o declara de forma explícita cuál cede, sin que ningún panel quede con ancho cero o negativo. Mientras el comportamiento no cambie, el test de ejemplo lo mantiene fijado y cualquier alteración del reparto lo hace fallar |
| L37 — **Un ancho de explorador negativo almacenado infla el panel de request.** `request_panel_width_from_cursor(400.0, -200.0)` devuelve 542 en lugar de colapsar al mínimo, porque el ancho del explorador se **resta** sin sanear: restar un negativo suma. `workspace_panel_width` nunca se valida antes de usarse como desplazamiento. La sesión persistida es la vía por la que un valor así podría llegar (`restore_pending_session` copia `panel_sizes` tal cual). Comportamiento previamente desconocido. | `midway-desktop/src/app.rs`: `request_panel_width_from_cursor` (2664-2667). Fijado por `panel_resize_tests::request_panel_width_from_cursor_handles_degenerate_inputs` | Hito 1 (tarea 10). **Criterio de cierre:** o el ancho de explorador se sanea antes de usarse como desplazamiento, o la Property 5 de la tarea 10.4 documenta que el rango de entrada válido excluye los negativos y la restauración de sesión los descarta. Cualquiera de las dos salidas se registra acá; ninguna se aplica sin decisión del usuario |
| L38 — **`debug_pane_layout(NaN)` devuelve `Stacked` por semántica de IEEE 754, no por una rama escrita.** La decisión es un único `available_width >= DEBUG_HORIZONTAL_BREAKPOINT`, y toda comparación de orden con `NaN` es falsa, así que `NaN` cae en el `else`. El resultado es razonable, pero no es una decisión de diseño registrada: sale del tipo. Comportamiento previamente desconocido, ahora fijado por la Property 4. | `midway-desktop/src/app.rs`: `debug_pane_layout` (5569-5575, la comparación en 5570), `DEBUG_HORIZONTAL_BREAKPOINT = 560.0` (5504). Fijado por `debug_layout_tests::property_4_debug_pane_layout_is_total_and_decided_by_breakpoint` y `degenerate_and_non_finite_widths_have_a_defined_layout` | Hito 1 (tarea 10). **Criterio de cierre:** si el layout apilado deja de ser el destino seguro para un ancho no representable, la decisión pasa a ser una rama explícita con su razón escrita. Hasta entonces la Property 4 impide que la semántica cambie en silencio |
| L39 — **Los identificadores de modo `Debug` y `Test` se mantienen en inglés de forma deliberada.** Son las dos etiquetas de las tabs de modo del `Top_Bar` y nombran, además, dos variantes del tipo `TopBarMode` y dos ramas de ruteo de contenido en `main_content_pane`. Traducirlas cambiaría texto visible en un archivo que la tarea 11.3 no toca (`ui/top_bar.rs`) y desalinearía la etiqueta del nombre del modo que usan el fuente, los comentarios y los documentos de auditoría. Se conservan como identificadores del producto, no como olvido de traducción. | `midway-desktop/src/ui/top_bar.rs` (`mode_tab`, invocado con `"Debug"` y `"Test"`); variantes `TopBarMode::Debug` y `TopBarMode::Test` en `midway-desktop/src/app.rs` | Hito 2 (junto con la descomposición de `app.rs` y la unificación del resto de `ui/`). **Criterio de cierre:** o las dos etiquetas pasan a español junto con las etiquetas de sección de `ui/top_bar.rs` y `ui/workspace_panel.rs` en una sola pasada, o la decisión de conservarlas se eleva a decisión de producto en `docs/product-principles.md` y esta fila se cierra citándola |
| L40 — **Las etiquetas de sección del Workspace_Panel siguen en inglés**: "Environments", "Data", "History", "Diagnostics" y "App updates". Quedan fuera de la tarea 11.3 porque viven en dos archivos que su lista de archivos no incluye. No es una excepción por criterio de idioma, es alcance: son las mismas cadenas que `L31` ya describía. | `midway-desktop/src/ui/top_bar.rs` (`section_display_label`); `midway-desktop/src/ui/workspace_panel.rs` (`section_tabs`) | Hito 2 (junto con `L32`, que es la otra deuda del mismo panel). **Criterio de cierre:** las cinco etiquetas quedan en español en ambos archivos, con el test de `ui/top_bar.rs` que hoy fija `expected_section_label` actualizado a los valores nuevos |
| L41 — **Las etiquetas de las tabs de configuración y de respuesta siguen en inglés**: "Params", "Headers", "Auth", "Body" y "Tests" en el `Request_Composer`, y "Body", "Headers", "Cookies" y "Tests" en el `Response_Inspector`. La tarea 11.3 sí toca esos dos archivos y las deja igual a propósito: las nueve son términos del protocolo HTTP y del vocabulario de la herramienta que el resto del texto en español ya usa sin traducir ("Sin cookies almacenadas", "la tab Tests del request", "Sin body"), y el `tab_bar` compacta el espaciado para que las cinco etiquetas del composer quepan en un panel de unos 280 px, de modo que alargarlas ("Parámetros", "Autenticación", "Cabeceras") las volvería a apretar. | `midway-desktop/src/ui/request_composer.rs` (`config_tabs`); `midway-desktop/src/ui/response_inspector.rs` (`response_tabs`) | Hito 2. **Criterio de cierre:** o se acepta el conjunto como vocabulario del producto y se lo registra en `docs/product-principles.md`, o se traduce con una medición previa de que las etiquetas más largas siguen entrando en el ancho mínimo del panel |
| L42 — **`Run` sigue en inglés en el texto del modo Test**: "Sin ejecución en curso. Presioná Run para ejecutar la colección.". La frase se pasó a voseo, pero `Run` se conserva porque nombra el control que el Collection_Runner **no expone** en la interfaz (`L12`): traducirlo inventaría el nombre de un botón que todavía no existe. La cadena se resuelve cuando ese control se agregue y tenga una etiqueta real. | `midway-desktop/src/app.rs` (`test_content`) | Hito 4 (el mismo de `L12`, que es el que agrega el control). **Criterio de cierre:** el control de ejecución existe, tiene etiqueta en español, y el texto del estado sin ejecución la cita en vez de citar `Run` |
| L43 — **Términos de protocolo y de formato conservados en inglés en las opciones del `Request_Composer`**: `Bearer`, `Basic` y `API Key` (esquemas de autenticación HTTP), `Header` (parte del mensaje HTTP), `JSON` y `Form data` (formatos de payload, el segundo por `multipart/form-data`), `Status` y `JSON Pointer` (RFC 6901), y `environment` dentro de "Sin environment". Se conservan porque son nombres propios de especificaciones o el término de dominio que el resto del texto en español ya usa; traducirlos solo en estas opciones rompería la consistencia en vez de arreglarla. Lo que sí pasó a español en la tarea 11.3 es todo lo demás de esas listas. | `midway-desktop/src/ui/request_composer.rs` (`impl Display` de `AuthKind`, `ApiKeyPlacementOption`, `BodyModeOption`, `AssertionSourceOption`; `SIN_ENVIRONMENT_LABEL`) | Hito 2. **Criterio de cierre:** la lista queda ratificada en `docs/product-principles.md` como vocabulario técnico del producto, o se reduce con una decisión escrita término por término. Mientras no cambie, `language_tests::retired_english_labels_do_not_come_back` impide que vuelvan las etiquetas que sí se tradujeron |

## 3. Alcance diferido

`Diferido` significa: alcance reconocido, **no** implementado en este spec, con hito
propietario nombrado. Cada fila lleva una línea de alcance y su hito, sin diseño (Req 12.6).

| Elemento | Estado | Hito propietario |
| --- | --- | --- |
| Capa de adaptadores multiprotocolo: abstraer el envío de request más allá del adaptador HTTP único de `infra/http_reqwest.rs`. | Diferido | Hito 7 |
| Soporte de GraphQL. | Diferido | Hito 7 |
| Soporte de gRPC. | Diferido | Hito 7 |
| Soporte de WebSocket. | Diferido | Hito 7 |
| Soporte de SSE. | Diferido | Hito 7 |
| Soporte de MQTT. | Diferido | Hito 7 |
| Soporte de SOAP. | Diferido | Hito 7 |
| Soporte de MCP. | Diferido | Hito 7 |
| Mocks de servicios. | Diferido | Hito 8 |
| Monitores de ejecución programada. | Diferido | Hito 8 |
| Flows (encadenado de requests con dependencias entre pasos). | Diferido | Hito 8 |
| Proxy de captura de tráfico. | Diferido | Hito 8 |
| Plugins WASM. | Diferido | Hito 9 |
| Requests asistidos por IA. | Diferido | Hito 9 |
| Scripting con Boa. | Diferido | Hito 9 |
| Rediseño de `ResponseBody` a bytes, con `body_text: String` reemplazado por una representación en bytes (ver L22, L23 y el ADR 0005). | Diferido | Hito 5 |
| Re-arquitectura de la persistencia: esquema versionado, respaldo previo, migración transaccional, validación posterior, rollback y tests desde versiones anteriores (ver L25 y L26). | Diferido | Hito 6 |

## 4. Qué falta registrar y quién lo registra

Las dos entradas previstas están resueltas y quedan cerradas más abajo: la primera por la
tarea 4.8 (`L35` a `L38`) y la segunda por la tarea 11.3 (`L39` a `L43`). No queda nada
pendiente de registrar en esta sección; si una tarea posterior del spec descubriera una
limitación nueva, la fila va a la §2 continuando la numeración desde `L44` y se anota acá
quién la registró.

**Fallos de Test_Caracterización sobre el baseline sin cambios (tarea 4.8, Req 6.7) —
cerrado.** Los Test_Caracterización de las tareas 4.1 a 4.7 ya existen y la tarea 4.8 los
ejecutó con `cargo test --workspace --all-features` sobre el baseline sin cambios de
producción. Resultado: **323 tests, 0 fallos, 0 ignorados, exit code 0**. **Ningún
Test_Caracterización falla sobre el baseline**, así que no se registró ningún defecto por
fallo de test, y se declara explícitamente en lugar de dejarlo implícito (la corrida
completa está transcrita en `docs/product-audit.md` §3.14).

Lo que sí produjo la escritura de esos tests son **cuatro comportamientos del baseline
previamente no documentados**, que el Req 6.6 clasifica como "comportamiento previamente
desconocido" y que quedan registrados como `L35` a `L38` en la §2, cada uno con ubicación,
hito propietario y criterio de cierre. Ninguno se descubrió por un fallo: se descubrió al
tener que escribir el valor esperado y encontrar que el valor observado no era el que la
función parecía prometer. No se ajustó ningún test para que pase ni se modificó producción
para volverlo verde (Req 6.6): los cuatro comportamientos quedan **fijados tal como son**,
de modo que cualquier cambio futuro los haga fallar de forma visible.

**Excepciones de idioma que quedan en pie (tarea 11.3, Req 9.6) — cerrado.** La mezcla de
idiomas del baseline está registrada como `L31`, y las excepciones que la tarea 11.3 deja
en pie quedan registradas como `L39` a `L43` en la §2, cada una con ubicación, hito
propietario y criterio de cierre. Son cinco y no una porque al implementar la unificación
apareció más de un motivo distinto para no traducir, y meterlos en una sola fila habría
tapado que cada uno se cierra por una vía diferente: `L39` (identificadores de modo) y
`L40` (etiquetas de sección) son **alcance** —viven en `ui/top_bar.rs` y
`ui/workspace_panel.rs`, que la tarea no toca—; `L41` (etiquetas de tab) y `L43` (términos
de protocolo y de formato) son **criterio de vocabulario** en archivos que la tarea sí
toca; `L42` (`Run`) es una **dependencia**: la palabra nombra un control que todavía no
existe (`L12`).

Estado de `L31` tras la tarea 11.3. `L31` es un registro del baseline y se deja tal cual;
lo que cambia es cuánto de lo que enumera sigue vigente. De sus cuatro grupos:

- **"Send"/"Sending..." — resueltos.** Ahora son "Enviar" y "Enviando…" en
  `ui/request_composer.rs`.
- **Encabezado "Collection Runner" — resuelto.** Ahora es "Ejecutor de la colección" en
  `app.rs`. La palabra `Run` de la línea de estado del mismo modo sí sigue en inglés, y por
  eso está en `L42` en vez de darse por cerrada con el encabezado.
- **Etiquetas de sección "Environments"/"Data"/"History"/"Diagnostics"/"App updates" —
  siguen vigentes**, ahora también con su propia fila (`L40`) porque tienen un hito y un
  criterio de cierre propios.
- **Identificadores de modo "Debug"/"Test" — siguen vigentes** de forma deliberada,
  registrados en `L39`.

Además de lo que `L31` enumeraba, la tarea 11.3 tradujo texto visible que ese registro no
había listado: los veredictos "PASSED"/"FAILED" del inspector de respuesta, las opciones de
los `pick_list` de auth, body, form data, origen y operador de assertion, la opción
"No Environment", los placeholders "Key"/"Value"/"Expected"/"Source"/"Operator", el
encabezado "Preview" del preview drawer y la línea de progreso del Collection_Runner, que
mostraba el `Debug` derivado de `CollectionRunProgressEvent` —con nombres de campo en
inglés— en texto visible. Nada de eso queda como excepción: por eso no tiene fila en la §2.
