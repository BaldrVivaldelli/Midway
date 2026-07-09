# Resumen de equivalencia funcional por fase (Migración Tauri → iced)

Este documento es el **resumen escrito de equivalencia funcional** exigido por el
Requisito 10 (Criterio 10.5) y por el Requisito 11 (Criterio 11.3) del spec
`tauri-to-iced-migration`. Para cada funcionalidad migrada de Midway_Tauri
(Tauri + React/TypeScript) a Midway_Desktop (100% Rust sobre `iced`), se indica
si el comportamiento observable quedó:

- **Equivalente** — el comportamiento observable coincide con el de Midway_Tauri
  para las mismas entradas.
- **Parcial** — el comportamiento observable difiere de Midway_Tauri en al menos
  un escenario documentado.
- **Resuelto distinto** — se decidió deliberadamente resolver la funcionalidad de
  otra forma respecto a la versión Tauri; se incluye la razón.

Cuando una funcionalidad se implementó de forma manual por ausencia de un crate
publicado apto (crates.io, no archivado/deprecado, con release en los últimos 12
meses), se documenta explícitamente esa ausencia de alternativa conforme al
Criterio 11.3.

> Alcance y estado: este resumen cubre las Fases 0 a 8 según lo descrito e
> implementado en `tasks.md` y `design.md`. Es la **base escrita** sobre la que se
> construyen la comparación funcional manual final (Tarea 17.2 → `docs/functional-comparison.md`)
> y el reporte de diferencias clasificadas (Tarea 17.3). La verificación manual
> ejecutada contra la última versión funcional de Midway_Tauri se consolida en
> esos documentos.

## Principio general de la migración

La lógica de dominio, infraestructura y runtime (`domain/`, `infra/`, `runtime/`)
se **extrajo sin reescritura** a `midway-core` y se consume en proceso desde
`midway-desktop`, eliminando la capa de comandos IPC de Tauri (`commands/mod.rs`).
Por diseño, toda funcionalidad respaldada por esas capas es **equivalente por
construcción**: se ejecuta exactamente el mismo código Rust que ya usaba
Midway_Tauri, cambiando únicamente el punto de invocación (llamada directa en
lugar de comando IPC) y el mecanismo de notificación (mensajes `iced` en lugar de
`app.emit`). Las diferencias potenciales se concentran en la lógica UI-adyacente
que vivía en TypeScript (`src/lib/*.ts`) y que sí debió reescribirse en Rust; esa
lógica se cubre con tests de equivalencia y de propiedad (`proptest`).

| Punto de acoplamiento a Tauri | Resolución | Estado |
| --- | --- | --- |
| `runtime/secret_executor.rs` usaba `tauri::async_runtime::spawn` | Reemplazado por `tokio::spawn` (Tokio ya era dependencia directa), sin cambiar el comportamiento observable del executor | Equivalente |
| `commands/mod.rs` + `state.rs` (orquestación IPC) | Reimplementados como funciones libres en `midway-desktop` que invocan `midway-core` y despachan `Message` en el ciclo `update` | Equivalente |

---

## Fase 0 — Reestructuración como Cargo workspace

Funcionalidad migrada: preparación estructural (sin cambio de comportamiento de
cara al usuario).

| Elemento | Estado | Notas |
| --- | --- | --- |
| Extracción de `domain/`/`infra/`/`runtime/` a `midway-core` | Equivalente | Solo se ajustaron rutas de módulos y visibilidad `pub`; no se reescribió lógica (Req 1.2, 1.9). |
| `midway` (Tauri) delega en `midway-core` | Equivalente | `commands/mod.rs` y `state.rs` siguen siendo Tauri-específicos pero delegan la orquestación; comportamiento de comandos sin cambios (Req 1.5). |
| Independencia de `midway-core` respecto a Tauri | Equivalente | Verificado por la Propiedad 28 (test determinístico sobre `cargo metadata`): `midway-core` no depende de `tauri` ni de `tauri-*` (Req 1.10). |
| Versión de `iced` pinneada | Equivalente | Versión exacta en `Cargo.toml`/`Cargo.lock`, sin rangos (Req 1.5, 11.1). |

Sin ausencia de crate que documentar en esta fase.

---

## Fase 1 — Request composer y response inspector

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Selector de método, barra de URL, botón Send, selector de environment, ícono de settings | Equivalente | Fila superior reconstruida con `pick_list` + `text_input` + `button`; mismo conjunto de métodos GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS (Req 2.1–2.5). |
| Ejecución de Send | Equivalente | Invoca `resolve_request` (interpolación + auth) y `request_executor.execute` de `midway-core` vía `Task::perform`; mismo motor que Tauri (Req 2.17). |
| Response Inspector (status/tiempo/tamaño, tabs Body/Headers/Tests) | Equivalente | Renderiza sin transformación el `ResponseEnvelope` + `AssertionReport` producidos por `midway-core`; verificado con tests de passthrough (Req 2.10, 2.11). |
| Import de cURL (`curl.rs`) | Equivalente | Port directo de `src/lib/curl.ts`; equivalencia verificada por la **Propiedad 1** frente a la referencia TypeScript (Req 2.6, 2.16). |
| Enrutamiento de pegado de cURL (tab vacía vs. no vacía; texto plano; malformado) | Equivalente | Reglas replicadas y verificadas por las **Propiedades 2, 3 y 4** (Req 2.7, 2.8, 2.9, 2.19). |
| Editor de texto (`ui/text_editor.rs`): resaltado JSON, formateo, lint, búsqueda | Parcial (widget) / Equivalente (comportamiento) | Ver detalle abajo. |

Detalle del Text_Editor_Component:

- **Resaltado de sintaxis JSON**: se usa el crate publicado `iced_highlighter`
  (cumple Criterio 11.2), reemplazando a CodeMirror (`CodeEditor.tsx`).
- **Formateo JSON**: `serde_json::from_str` + `to_string_pretty`; idempotencia y
  preservación de valor verificadas por la **Propiedad 5** (Req 2.13).
- **Lint JSON**: usa `serde_json::Error.line()/column()`; posición exacta
  verificada por la **Propiedad 6** (Req 2.14).
- **Búsqueda de texto**: **implementada de forma manual** sobre el `Content` del
  editor (coincidencias case-insensitive + navegación next/previous), verificada
  por la **Propiedad 7** (Req 2.15).
  - **Ausencia de alternativa (Criterio 11.3)**: no existe en crates.io un widget
    de búsqueda in-editor para `iced` publicado, no archivado/deprecado y con
    release en los últimos 12 meses; por ello se implementó manualmente. Esto
    reemplaza la búsqueda que en Tauri proveía CodeMirror. Comportamiento
    equivalente al observable por el usuario.

- **Preview del request** (drawer de settings): Equivalente. Muestra
  `domain::preview::make_preview` y `make_curl_command` de `midway-core` sin
  reescritura; fidelidad verificada por la **Propiedad 8** (Req 2.18).

---

## Fase 2 — Tabs de configuración del request

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Tabs Params/Headers/Auth/Body/Tests | Equivalente | Implementadas con `iced_aw::{TabBar, Tabs}` (crate publicado apto, Criterio 11.2), reutilizadas también en Response y Workspace_Panel (Req 3.1). |
| Tab por defecto según método HTTP + override manual | Equivalente | Params para GET/HEAD/OPTIONS, Body para POST/PUT/PATCH/DELETE, respetando `manual_tab_override`; verificado por la **Propiedad 9** (Req 3.2, 3.3). |
| Tab Auth (None/Bearer/Basic/ApiKey) | Equivalente | `pick_list` sobre `domain::http::AuthConfig`; al cambiar de tipo se reemplaza la variante por su default vacío; descarte de campos verificado por la **Propiedad 11** (Req 3.4–3.7, 3.10). |
| Editor de filas key/value (Params/Headers) | Equivalente | Reutiliza `domain::http::KeyValueRow`; CRUD + toggle `enabled` verificado contra modelo de referencia por la **Propiedad 10** (Req 3.8). |
| Tab Tests (assertions) | Equivalente | Editor de `domain::testing::ResponseAssertion` que invoca `evaluate_response_assertions` (motor existente, sin reescritura); cubierto por unit tests source/operator (Req 3.9). |

Sin ausencia de crate que documentar en esta fase.

---

## Fase 3 — Workspace panel lateral

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Secciones Environments/Data/History/Diagnostics/App updates | Equivalente | Panel lateral colapsable con `iced_aw::Tabs` (Req 4.1). |
| CRUD de Environments + límites (máx. 100, nombre ≤ 100, un activo) | Equivalente | CRUD sobre `EnvironmentRecord` vía `SqliteRepository`. Nota: la validación de duplicados y límites se ubica en la capa de orquestación de `midway-desktop` **igual que en Tauri se haría en `commands/mod.rs`** (la infra no impone el límite); comportamiento equivalente. Verificado por Propiedades 12, 15, 16 (Req 4.2, 4.9, 4.10). |
| Export (nativo v1, Postman v2.1) | Equivalente | Reutiliza `domain::interop::export_postman_collection` y `make_native_bundle` sin reescritura (Req 4.3). |
| Import (nativo v1, Postman v2.1, OpenAPI v3 JSON/YAML) + límite 10 MB | Equivalente | Reutiliza los parsers de `domain::interop`; el límite de 10 MB se valida antes de parsear. Validación por formato/tamaño verificada por la **Propiedad 13** (Req 4.4, 4.5). |
| Resolución de colisión de nombres en import | Equivalente | Conserva ambos elementos renombrando el importado; verificado por la **Propiedad 17** (Req 4.11). |
| History (máx. 500) | Resuelto distinto (ajuste de límite) | Se introduce `HISTORY_LIMIT = 500` en `midway-desktop`. **Razón**: `commands/mod.rs` en Tauri retenía 50 entradas; el requisito exige 500, por lo que se elevó deliberadamente el límite. Orden por recencia verificado por la **Propiedad 14** (Req 4.6). |
| Diagnostics (máx. 200) | Resuelto distinto (mecanismo de persistencia) | Port de `src/lib/diagnostics.ts`. **Razón**: en Tauri los crashes se guardaban en `window.localStorage`; sin navegador, se persisten hasta 200 `CrashRecord` en un archivo JSON en el data dir de la app. Comportamiento observable (lista acotada, orden por recencia) equivalente; verificado por la **Propiedad 14** y unit tests nominal/borde (Req 4.7). |
| App updates (estado del Updater) | Equivalente | Card de estado que reemplaza a `UpdateCenterCard.tsx`; la lógica de red llega en la Fase 6 (Req 4.8). |

Sin ausencia de crate que documentar en esta fase (`iced_aw` cubre la necesidad de tabs).

---

## Fase 4 — Collection runner

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Ejecución secuencial de requests guardados | Equivalente | `collection_runner::run_collection` migrado desde el comando `run_collection`, reutilizando `domain::runner` sin reescritura; mismo orden de ejecución (Req 5.1). |
| Progreso incremental | Resuelto distinto (transporte) | **Razón**: se sustituye `app.emit(COLLECTION_RUN_PROGRESS_EVENT, ...)` de Tauri por mensajes de progreso vía canal `mpsc` consumidos por una `iced::subscription`. La semántica de progreso (inicio/fin de request, contadores) es equivalente; solo cambia el mecanismo de transporte (Req 5.2). |
| Reporte consolidado + environment de override | Equivalente | Éxito/fallo, status, tiempo, resultados de assertions y totales; usa environment de override o activo por defecto (Req 5.3, 5.4, 5.6). |
| Fallo por request sin detener la secuencia | Equivalente | Registra motivo y continúa (Req 5.5). |
| Colección vacía (N=0) | Equivalente | Reporte inmediato con cero requests, sin llamadas HTTP (Req 5.7). |
| Cancelación en curso | Equivalente | `oneshot::Sender<()>` compartido, replicando el patrón de `RequestExecutorHandle::cancel`; el reporte refleja solo lo completado (Req 5.8). |

Las ocho reglas anteriores están cubiertas por la **Propiedad 18** (mock de executor, sin HTTP real). Sin ausencia de crate que documentar en esta fase.

---

## Fase 5 — Command palette, persistencia de sesión y shortcuts

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Command Palette (scoring) | Equivalente | Port directo de `src/lib/commandPalette.ts` con pesos idénticos (exacto/prefijo/substring sobre título, keywords, subtítulo); cubierto por unit tests nominal/sin coincidencias (Req 6.1). |
| Apertura/cierre (Ctrl/Cmd+K) y ejecución de ítems | Equivalente | Overlay vía `iced::widget::stack`; ejecuta la acción y cierra (Req 6.1, 6.2). |
| Autosave de sesión (≤ 2 s) | Resuelto distinto (mecanismo) / Equivalente (garantía) | `SessionSnapshot` a `session.json` con escritura atómica (temporal + `rename`), disparado por `iced::time::every`. **Razón**: no hay `localStorage`; se persiste en el data dir. La garantía de ≤ 2 s desde la última modificación se verifica con reloj simulado en la **Propiedad 19** (Req 6.3). |
| Restauración de sesión al reabrir | Equivalente | Restaura tabs abiertas, tab activa y tamaños de panel; round-trip verificado por la **Propiedad 20** (Req 6.4, 6.7). |
| Stack de tabs cerradas (máx. 20, LIFO) | Equivalente | `VecDeque` acotado; verificado por la **Propiedad 21** (Req 6.5). |
| Aviso de unsaved changes (Guardar/Descartar/Cancelar) | Equivalente | Modal overlay; acciones verificadas por la **Propiedad 22** (Req 6.6). |
| Shortcuts de teclado | Equivalente | `iced::keyboard::on_key_press` mapea la tabla completa del Criterio 6.8 (Ctrl+Enter, Ctrl+S, Ctrl+Shift+N/P, Ctrl+., Ctrl+K, Ctrl+W, Ctrl+Shift+T, Alt+1..9, Esc); cubierto por unit tests (Req 6.8). |
| Error Boundary por componente | Resuelto distinto (mecanismo) / Equivalente (efecto) | **Razón**: `iced` no tiene `componentDidCatch` como React; se envuelve cada handler de `update`/cierre de `view` aislado con `std::panic::catch_unwind`, se registra un `CrashRecord` (`source = ComponentBoundary`) y se sustituye el componente por un mensaje de error sin derribar la app. Aislamiento verificado por la **Propiedad 23** (Req 6.9). |
| Descarte seguro de sesión corrupta/incompatible | Equivalente | Ante fallo de deserialización o `version` incompatible: descarta, inicia sesión vacía y notifica; verificado por la **Propiedad 24** (Req 6.10). |

**Ausencia de alternativa (Criterio 11.3)**: no existe un crate publicado apto que
provea un `Session_Store` con autosave/atomicidad/stack de tabs para `iced`; se
implementó manualmente (`session.rs`). Igualmente, la recuperación de panics por
componente carece de un crate equivalente para `iced` y se implementó con
`catch_unwind`.

---

## Fase 6 — Updater in-app

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Descarga de manifiesto + comparación semver | Equivalente | `updater.rs` descarga `latest.json`/`latest-beta.json` según canal y compara con `env!("CARGO_PKG_VERSION")` usando el crate `semver` (pinneado); disponibilidad verificada por la **Propiedad 25** (Req 7.1, 7.2). |
| Descarga con progreso porcentual (≥ 1/seg) | Equivalente | `reqwest::Response::bytes_stream`; frecuencia y 100% final verificados por test de integración con servidor local (Req 7.3). |
| Verificación SHA256 como compuerta | Equivalente | Crate `sha2` (pinneado) contra `SHA256SUMS.txt`; si coincide instala, si no descarta sin sobrescribir y muestra error de corrupción; compuerta verificada por la **Propiedad 26** (Req 7.4–7.6). |
| Relanzamiento post-instalación + continuidad si se declina | Equivalente | Ofrece relanzar; si se declina, mantiene la sesión con la versión previa y usa la nueva en el siguiente inicio (Req 7.7, 7.8). |
| Fallo de verificación/descarga preserva la versión instalada | Equivalente | Mensaje de error y versión operativa; verificado por la **Propiedad 27** (Req 7.9). |

**Resuelto distinto + Ausencia de alternativa (Criterios 7.1 y 11.3)**: se
reemplaza `tauri-plugin-updater` por una implementación manual sobre `reqwest`.
**Razón**: no existe en crates.io un crate publicado, no archivado/deprecado y con
release en los últimos 12 meses que implemente el protocolo específico de Midway
(manifest `latest.json`/`latest-beta.json` + `SHA256SUMS.txt` ya publicados por el
pipeline de release existente). Crates como `self_update` asumen el modelo
"GitHub Releases + naming convention" y no son compatibles sin adaptar el formato
de manifest ya publicado. El comportamiento observable (verificar, descargar,
validar checksum, instalar, relanzar) es equivalente al del updater de Tauri, y
consume los mismos artefactos sin modificarlos.

---

## Fase 7 — Empaquetado y distribución

| Funcionalidad | Estado | Notas / razón |
| --- | --- | --- |
| Herramienta de empaquetado | Resuelto distinto | Se reemplaza `tauri-build`/Tauri CLI por `cargo-packager` (versión pinneada, release dentro de los últimos 12 meses — cumple Criterio 8.1). **Razón**: al eliminar Tauri, se requiere un empaquetador nativo de Rust para generar instaladores Windows (NSIS/MSI) y Linux (AppImage/deb) de `midway-desktop`. |
| Generación de `latest.json`/`latest-beta.json`/`SHA256SUMS.txt` | Equivalente | `scripts/release/generate-updater-json.mjs` y `generate-checksums.mjs` se preservan casi sin cambios (son agnósticos al empaquetador); solo se ajustan los patrones de nombre de archivo a los de `cargo-packager` (Req 8.3, 8.4). |
| Configuración de release por canal (estable/beta) | Equivalente | `render-tauri-config.mjs` se reemplaza por un script equivalente que preserva identifier, productName, homepage y publisher (Req 8.5). |
| Workflows CI/release | Equivalente (adaptado) | Se reemplaza `tauri-apps/tauri-action` por `cargo packager` sobre `midway-desktop`, manteniendo matriz Windows/Linux x86_64 y firma/checksum (Req 8.5). |
| Detención del pipeline ante fallo de empaquetado | Pendiente de cierre | Tareas 15.5/15.6 en curso; ver estado en `tasks.md`. Cuando esté implementado: al fallar `cargo-packager` en una plataforma se detiene el pipeline, se reporta plataforma/causa y se evita publicar artefactos parciales (Req 8.6). |

**Nota de estado**: la Fase 7 aún no está completamente cerrada en `tasks.md`
(tareas 15.5 y 15.6 pendientes). Este punto debe reflejarse en el reporte de
diferencias (Tarea 17.3) hasta su finalización.

---

## Fase 8 — Limpieza final

Funcionalidad: eliminación del stack Tauri/React una vez verificada la paridad.
No introduce comportamiento nuevo de cara al usuario; su objetivo es dejar el
repositorio 100% Rust sin código muerto.

| Elemento | Estado esperado | Notas |
| --- | --- | --- |
| Eliminación del crate `midway` y `src-tauri/` | Equivalente (sin impacto funcional) | Solo tras verificar paridad funcional completa (Tareas 17.2–17.4). El binario de escritorio pasa a ser exclusivamente `midway-desktop` (Req 9.1, 9.4). |
| Eliminación de `src/` (TS/React) y tooling npm | Equivalente (sin impacto funcional) | `package.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `node_modules/`, `tests/ui/`, etc. (Req 9.2). |
| Compilación con crates restantes | Equivalente | `cargo check`/`build` sobre `midway-core` + `midway-desktop` (Req 9.3). |
| README y workflows sin referencias a React/TS/Vite/Tauri | Equivalente | Actualización de documentación y CI (Req 9.5, 8.5). |

**Compuerta de bloqueo (Criterio 10.8)**: la limpieza de la Fase 8 no debe
ejecutarse mientras exista una regresión bloqueante sin documentar ni aceptar
explícitamente. La verificación de esta compuerta es responsabilidad de la
Tarea 17.4, apoyada en el reporte de diferencias (Tarea 17.3).

---

## Resumen de decisiones "resueltas distinto" y ausencias de crate

| # | Área | Decisión | Razón |
| --- | --- | --- | --- |
| 1 | Búsqueda in-editor (Fase 1) | Implementación manual | Sin crate de búsqueda in-editor apto para `iced` (11.3). |
| 2 | History (Fase 3) | Límite elevado 50 → 500 | Requisito 4.6 exige 500; Tauri usaba 50. |
| 3 | Diagnostics (Fase 3) | Archivo JSON en data dir en vez de `localStorage` | No hay navegador; persistencia nativa. |
| 4 | Progreso del runner (Fase 4) | Canal `mpsc` + subscription en vez de `app.emit` | Sin sistema de eventos de Tauri; semántica equivalente. |
| 5 | Autosave de sesión (Fase 5) | `session.json` atómico en data dir | No hay `localStorage`; garantía ≤ 2 s preservada. |
| 6 | Error Boundary (Fase 5) | `catch_unwind` por componente | `iced` no tiene `componentDidCatch`; sin crate equivalente (11.3). |
| 7 | Updater (Fase 6) | Implementación manual sobre `reqwest` | Sin crate compatible con el manifest/checksums ya publicados (7.1, 11.3). |
| 8 | Empaquetado (Fase 7) | `cargo-packager` en vez de Tauri CLI | Al retirar Tauri se necesita empaquetador nativo de Rust (8.1). |

En todos los casos anteriores el **comportamiento observable por el usuario** se
mantiene equivalente al de Midway_Tauri; las diferencias son de mecanismo interno,
justificadas por la eliminación de la capa Tauri/navegador o por la ausencia de un
crate publicado apto conforme al Criterio 11.3.

## Documentos relacionados

- Comparación funcional manual final (Tarea 17.2): `docs/functional-comparison.md` *(pendiente)*.
- Reporte de diferencias clasificadas (Tarea 17.3): [`docs/differences-report.md`](./differences-report.md).
- Verificación de la compuerta de bloqueo (Tarea 17.4): [`docs/blocking-regression-gate.md`](./blocking-regression-gate.md).
- Distribución y release: `docs/distribution.md`.
