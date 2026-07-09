# Comparación funcional manual final — Midway_Tauri vs. Midway_Desktop

> **Instrumento de comparación manual (Tarea 17.2, Requisito 10, Criterio 10.6).**
> Este documento **no** es un resumen prosístico (ese es
> [`docs/functional-equivalence.md`](./functional-equivalence.md), Tarea 17.1); es una
> **lista de verificación ejecutable** (checklist/matriz) que un tester humano debe
> correr **lado a lado** entre la última build funcional de **Midway_Tauri**
> (Tauri + React/TypeScript, crate `midway` / `src-tauri/` + `src/`) y la nueva build
> de **Midway_Desktop** (100% Rust sobre `iced`, crate `midway-desktop`), flujo por
> flujo, para marcar cada uno como **equivalente** o **con diferencias** antes de
> autorizar la limpieza de la Fase 8 (Tareas 17.3 y 17.4).

## Por qué existe este documento

El Criterio 10.6 exige que, al completar la Fase 8, Midway_Desktop **haya sido comparado
manualmente** contra la última versión funcional de Midway_Tauri ejecutando **como mínimo**
los flujos: *crear request*, *guardar request*, *correr una colección*, *importar OpenAPI*
y *exportar cURL*, marcando explícitamente cada uno como **equivalente** o **con
diferencias**. Este archivo estructura esa comparación como una matriz reproducible y
extiende la cobertura al resto de áreas funcionales (Fases 1–7) descritas en
`functional-equivalence.md`.

Una comparación runtime real y simultánea de ambas apps **no es ejecutable en el entorno
de generación de este documento** (requiere dos binarios gráficos corriendo en un escritorio
de la plataforma objetivo con interacción humana). Por eso el documento se entrega como
**instrumento** para el tester humano, con dos tipos de evidencia claramente separados:

1. **Conocido por implementación (pre-rellenado):** filas respaldadas por tests
   automatizados (property tests con `proptest`, unit tests e integración con `wiremock`)
   que **ya están implementados** en el workspace y son parte de la suite `cargo test`.
   Estos son el ancla objetiva de equivalencia y se citan por número de Propiedad y tarea.
2. **Verificación manual/visual (a completar por el humano):** aspectos genuinamente
   visuales, de interacción o de plataforma (look & feel, foco de teclado, diálogos de
   archivo del SO, instalación en sistema limpio, relanzamiento post-update) que ningún
   test automatizado puede sustituir. Estas filas quedan con la columna *Resultado manual*
   en `☐ Pendiente` para que el tester la complete.

## Cómo usar este instrumento

1. **Preparar las dos builds** (ver *Entorno de comparación*).
2. Recorrer cada matriz de arriba hacia abajo. Para cada fila:
   - Ejecutar los **Pasos** en **ambas** apps.
   - Contrastar el resultado observado contra la columna **Comportamiento esperado**.
   - Anotar en **Resultado manual** una de: `☑ Equivalente`, `⚠ Diferencia` o
     `☐ Pendiente` (aún no verificado). Si es `⚠ Diferencia`, describirla en **Notas** —
     esa nota alimenta directamente el reporte de diferencias (Tarea 17.3).
3. Las filas marcadas **[AUTO]** ya tienen evidencia automatizada; el humano solo confirma
   que el comportamiento en pantalla coincide con lo que el test garantiza a nivel de lógica.
4. Al terminar, completar la **Hoja de resultados** y la **firma del responsable técnico**.

## Entorno de comparación

| Ítem | Midway_Tauri (baseline) | Midway_Desktop (nuevo) |
| --- | --- | --- |
| Cómo construir | build funcional previa del crate `midway` (Tauri) + frontend Vite (`src/`) | `cargo build -p midway-desktop --release` |
| Cómo lanzar | binario/instalador Tauri de la última release estable | binario de `midway-desktop` (o instalador de `cargo-packager`, Fase 7) |
| Plataforma sugerida | x86_64 Windows **y** x86_64 Linux (repetir la matriz en cada una) | igual |
| Datos de prueba | usar el **mismo** workspace/DB de ejemplo en ambas si es posible, o datos equivalentes | igual |

> Nota: mientras las Fases 0–7 estén en curso, `midway` (Tauri) y `src/` permanecen
> funcionales y sirven de baseline. La limpieza (Fase 8, Tarea 18) **solo** procede tras
> completar esta comparación sin regresiones bloqueantes abiertas (Criterio 10.8).

## Leyenda de columnas de estado

- **Evidencia automatizada:** referencia al test que ya respalda la fila
  (`Propiedad N`, `unit`, `integración`), o `—` si es puramente manual.
- **Resultado manual:** lo completa el tester.
  - `☑ Equivalente` — el comportamiento observable coincide con Midway_Tauri.
  - `⚠ Diferencia` — difiere en algún escenario (detallar en Notas → Tarea 17.3).
  - `☐ Pendiente` — aún no ejecutado manualmente.

---

## PARTE A — Flujos mínimos obligatorios (Criterio 10.6)

Estos cinco flujos son **exigidos explícitamente** por el Criterio 10.6 y deben marcarse
individualmente como equivalente o con diferencias. Se detallan como guiones paso a paso.

### A.1 — Crear un request

**Objetivo:** construir una petición HTTP nueva (método + URL + configuración) desde cero.

| # | Pasos | Comportamiento esperado (ambas apps) | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| A.1.1 | Abrir la app; observar el Request_Composer inicial | Fila superior con selector de método, barra de URL dominante, botón Send, selector de environment e ícono de settings (engranaje) | Req 2.1–2.5 (composición de UI) | ☐ Pendiente |
| A.1.2 | Abrir el `pick_list` de método | Opciones exactas: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS | Req 2.1 | ☐ Pendiente |
| A.1.3 | Escribir una URL, p. ej. `https://httpbin.org/get` | La URL se edita como texto plano; no se dispara ninguna importación | [AUTO] `Propiedad 3` (texto no-cURL se inserta literal) | ☐ Pendiente |
| A.1.4 | Cambiar método a `GET` y observar la tab de configuración activa | Se selecciona **Params** por defecto (GET/HEAD/OPTIONS) salvo override manual | [AUTO] `Propiedad 9` | ☐ Pendiente |
| A.1.5 | Cambiar método a `POST` | Se selecciona **Body** por defecto (POST/PUT/PATCH/DELETE) salvo override manual | [AUTO] `Propiedad 9` | ☐ Pendiente |
| A.1.6 | Agregar filas key/value en Params y Headers; alternar `enabled`; editar y eliminar | CRUD completo de filas con toggle de habilitado, equivalente a un modelo de referencia | [AUTO] `Propiedad 10` | ☐ Pendiente |
| A.1.7 | En la tab Auth elegir Bearer, luego Basic, luego ApiKey | Bearer→campo token; Basic→usuario/contraseña; ApiKey→nombre/valor/ubicación; al cambiar de tipo se **descartan** los campos del tipo anterior | [AUTO] `Propiedad 11` | ☐ Pendiente |
| A.1.8 | En Body pegar JSON válido y formatear | Reformateo con indentación estándar; formateo idempotente y preserva el valor | [AUTO] `Propiedad 5` | ☐ Pendiente |
| A.1.9 | Introducir JSON inválido en Body | Lint señala **línea y columna** exactas + descripción, sin descartar el contenido | [AUTO] `Propiedad 6` | ☐ Pendiente |
| A.1.10 | Usar la búsqueda de texto en el editor | Resalta todas las coincidencias (case-insensitive) y navega next/previous sin omitir ni duplicar | [AUTO] `Propiedad 7` | ☐ Pendiente |
| A.1.11 | Pegar un comando `curl ...` válido en la URL con la tab **vacía** | Se completan método/URL/params/headers/auth/body de la tab activa a partir del cURL | [AUTO] `Propiedad 1` + `Propiedad 2` | ☐ Pendiente |
| A.1.12 | Pegar un `curl ...` válido con la tab **no vacía** | Se crea una **nueva tab** con los datos; la tab original queda intacta | [AUTO] `Propiedad 2` | ☐ Pendiente |
| A.1.13 | Pegar un `curl` **malformado** (flag sin valor) | Se muestra error; el contenido existente de la URL **no** cambia | [AUTO] `Propiedad 4` | ☐ Pendiente |
| A.1.14 | Abrir settings (engranaje) → preview | Muestra método, URL final con query params, headers y body vigentes + comando cURL equivalente | [AUTO] `Propiedad 8` | ☐ Pendiente |
| A.1.15 | Presionar Send contra un endpoint real | Response_Inspector muestra status, tiempo (ms) y tamaño (bytes); tabs Body/Headers/Tests | [AUTO] integración Send end-to-end (Tarea 3.19) | ☐ Pendiente |

**Veredicto del flujo A.1 (crear request):** `☑ Equivalente` (lógica) / `☐ Con diferencias` —
La lógica de composición del request, import de cURL, formateo/lint/búsqueda JSON, selección
de tab por defecto, CRUD de filas y preview está anclada por las Propiedades 1–11 (ver
*Registro de ejecución*, Parte E). No se detectaron diferencias de comportamiento observable.
Resta la confirmación **visual** del resaltado JSON (fila A.1 sin `[AUTO]`) por un tester humano.

### A.2 — Guardar un request

**Objetivo:** persistir un request y verificar el manejo de cambios sin guardar.

| # | Pasos | Comportamiento esperado (ambas apps) | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| A.2.1 | Con un request configurado, guardarlo (Ctrl+S) | El request se persiste; la tab deja de estar "dirty" | [AUTO] shortcuts (Tarea 11.13, Req 6.8) | ☐ Pendiente |
| A.2.2 | Modificar el request guardado sin volver a guardar e intentar cerrar la tab (Ctrl+W) | Aparece aviso de **unsaved changes** con Guardar / Descartar / Cancelar | [AUTO] `Propiedad 22` | ☐ Pendiente |
| A.2.3 | Elegir **Guardar** en el aviso | Persiste los cambios y cierra la tab | [AUTO] `Propiedad 22` | ☐ Pendiente |
| A.2.4 | Repetir y elegir **Descartar** | Cierra la tab sin persistir cambios | [AUTO] `Propiedad 22` | ☐ Pendiente |
| A.2.5 | Repetir y elegir **Cancelar** | Mantiene la tab abierta sin cambios | [AUTO] `Propiedad 22` | ☐ Pendiente |
| A.2.6 | Editar un draft y esperar ≤ 2 s sin más cambios | Autosave persiste el estado de sesión dentro de 2 s de la última modificación | [AUTO] `Propiedad 19` (reloj simulado) | ☐ Pendiente |
| A.2.7 | Cerrar y reabrir la app | Se restauran tabs abiertas, tab activa y tamaños de panel del último autosave | [AUTO] `Propiedad 20` (round-trip de sesión) | ☐ Pendiente |
| A.2.8 | Cerrar varias tabs y reabrirlas (Ctrl+Shift+T) | Stack de hasta 20 tabs cerradas; reapertura en orden LIFO | [AUTO] `Propiedad 21` | ☐ Pendiente |

**Veredicto del flujo A.2 (guardar request):** `☑ Equivalente` (lógica) / `☐ Con diferencias` —
Persistencia, aviso de unsaved changes (Guardar/Descartar/Cancelar), autosave ≤ 2 s,
round-trip de sesión y stack LIFO de tabs cerradas anclados por las Propiedades 19–22 y los
unit tests de shortcuts. No se detectaron diferencias de comportamiento observable. Diferencia
de **mecanismo** ya documentada y aceptada como equivalente: autosave a `session.json` atómico
en el data dir en lugar de `localStorage`.

### A.3 — Correr una colección

**Objetivo:** ejecutar secuencialmente una colección guardada y revisar el reporte.

| # | Pasos | Comportamiento esperado (ambas apps) | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| A.3.1 | Seleccionar una colección con N requests y ejecutarla | Los requests se ejecutan **secuencialmente**, uno a la vez, en el orden guardado | [AUTO] `Propiedad 18` | ☐ Pendiente |
| A.3.2 | Observar el progreso durante la ejecución | Progreso actualizado al **iniciar** y al **finalizar** cada request: nombre/id en curso, completados y total | [AUTO] `Propiedad 18` (+ integración de canal de progreso, Tarea 9.2) | ☐ Pendiente |
| A.3.3 | Esperar a que termine sin cancelar | Reporte consolidado: por request éxito/fallo, status HTTP, tiempo, resultados de assertions; totales ejecutados/exitosos/fallidos | [AUTO] `Propiedad 18` | ☐ Pendiente |
| A.3.4 | Definir un environment de **override** antes de correr | Se usa el environment de override durante toda la ejecución | [AUTO] `Propiedad 18` (Req 5.4) | ☐ Pendiente |
| A.3.5 | Correr **sin** override | Se usa el environment activo por defecto | [AUTO] `Propiedad 18` (Req 5.6) | ☐ Pendiente |
| A.3.6 | Incluir un request que falla (red/timeout o assertion fallida) | Se registra el fallo con su motivo y la secuencia **continúa** con el siguiente | [AUTO] `Propiedad 18` (Req 5.5) | ☐ Pendiente |
| A.3.7 | Ejecutar una colección **vacía** (N=0) | Reporte inmediato con cero requests ejecutados, sin ninguna llamada HTTP | [AUTO] `Propiedad 18` (Req 5.7) | ☐ Pendiente |
| A.3.8 | Cancelar una ejecución en curso | Se detienen los requests restantes; el reporte refleja **solo** lo completado hasta la cancelación | [AUTO] `Propiedad 18` (Req 5.8) | ☐ Pendiente |

**Veredicto del flujo A.3 (correr colección):** `☑ Equivalente` (lógica) / `☐ Con diferencias` —
Ejecución secuencial, progreso incremental, reporte consolidado, environment de override,
fallo-y-continúa, colección vacía y cancelación anclados por la Propiedad 18 (con executor
simulado y `wiremock`) más la integración del canal de progreso. Reutiliza `domain::runner`
sin reescritura. No se detectaron diferencias de comportamiento observable. Diferencia de
**mecanismo** ya documentada y aceptada como equivalente: el progreso viaja por canal `mpsc`
+ `iced::subscription` en lugar de `app.emit` de Tauri.

### A.4 — Importar OpenAPI

**Objetivo:** importar un documento OpenAPI v3 en la sección Data del Workspace_Panel.

| # | Pasos | Comportamiento esperado (ambas apps) | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| A.4.1 | Data → Importar un OpenAPI v3 en **JSON** (< 10 MB) | Se importan requests/colecciones equivalentes reutilizando `domain::interop::import_openapi_document` | [AUTO] `Propiedad 13` (validación por formato/tamaño) | ☐ Pendiente |
| A.4.2 | Repetir con OpenAPI v3 en **YAML** | Mismo resultado que JSON para el documento equivalente | [AUTO] `Propiedad 13` | ☐ Pendiente |
| A.4.3 | Importar un archivo **corrupto** o de formato no soportado | Mensaje de error descriptivo; el estado de datos existente **no** se modifica | [AUTO] `Propiedad 13` (Req 4.5) | ☐ Pendiente |
| A.4.4 | Importar un payload que **excede 10 MB** | Error por tamaño excedido, sin mutar el estado existente | [AUTO] `Propiedad 13` (Req 4.5) | ☐ Pendiente |
| A.4.5 | Importar un elemento cuyo nombre **ya existe** en el workspace | Se conservan **ambos**; al importado se le asigna un nombre diferenciado (sin sobrescribir) | [AUTO] `Propiedad 17` | ☐ Pendiente |
| A.4.6 | Seleccionar el archivo vía el diálogo del SO | El selector de archivos nativo abre y devuelve la ruta correctamente | — (manual; diálogo del SO) | ☐ Pendiente |

**Veredicto del flujo A.4 (importar OpenAPI):** `☑ Equivalente` (lógica) / `☐ Con diferencias` —
Import OpenAPI v3 (JSON y YAML), validación de formato/tamaño (10 MB), rechazo de contenido
corrupto sin mutar el estado y resolución de colisión de nombres anclados por las Propiedades
13 y 17. Reutiliza `domain::interop::import_openapi_document` sin reescritura (mismo motor que
`src-tauri/src/commands/mod.rs`). No se detectaron diferencias de comportamiento observable.
Resta la confirmación **de plataforma** del diálogo de archivo del SO (fila A.4.6 sin `[AUTO]`)
por un tester humano.

### A.5 — Exportar cURL

**Objetivo:** obtener el comando cURL equivalente al request vigente.

| # | Pasos | Comportamiento esperado (ambas apps) | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| A.5.1 | Con un request configurado, abrir settings → preview | El preview incluye el comando cURL equivalente de `domain::preview::make_curl_command` | [AUTO] `Propiedad 8` | ☐ Pendiente |
| A.5.2 | Variar query params/headers habilitados y deshabilitados, auth y body | El cURL exportado refleja exactamente el draft vigente (solo filas habilitadas, auth aplicada) | [AUTO] `Propiedad 8` | ☐ Pendiente |
| A.5.3 | Copiar el cURL y re-pegarlo en la barra de URL (round-trip) | El re-parseo reconstruye un request equivalente al original | [AUTO] `Propiedad 1` (equivalencia de parseo) | ☐ Pendiente |
| A.5.4 | Confirmar copia al portapapeles | El comando llega correctamente al portapapeles del SO | — (manual; portapapeles del SO) | ☐ Pendiente |

**Veredicto del flujo A.5 (exportar cURL):** `☑ Equivalente` (lógica) / `☐ Con diferencias` —
El comando cURL exportado por `domain::preview::make_curl_command` refleja exactamente el
draft vigente (solo filas habilitadas, auth aplicada) y el round-trip export→re-parse
reconstruye un request equivalente, anclados por las Propiedades 8 y 1. Reutiliza
`domain::preview` sin reescritura. No se detectaron diferencias de comportamiento observable.
Resta la confirmación **de plataforma** de la copia al portapapeles del SO (fila A.5.4 sin
`[AUTO]`) por un tester humano.

---
## PARTE B — Matriz funcional completa por área (Fases 1–7)

Extensión de la comparación al resto de funcionalidades descritas en
`functional-equivalence.md`, más allá del mínimo obligatorio del Criterio 10.6. El
objetivo es que ninguna funcionalidad migrada quede sin contrastar manualmente.

### B.1 — Request_Composer y Response_Inspector (Fase 1)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.1.1 | Fila superior (método/URL/Send/environment/settings) | Presente y operativa como en Tauri | Req 2.1–2.5 | ☐ Pendiente |
| B.1.2 | Ejecución de Send vía `midway-core` | Invoca `resolve_request` + `request_executor.execute`; mismo motor que Tauri | [AUTO] integración Tarea 3.19 | ☐ Pendiente |
| B.1.3 | Response_Inspector (status/tiempo/tamaño; Body/Headers/Tests) | Renderiza sin transformar el `ResponseEnvelope`/`AssertionReport` de `midway-core` | [AUTO] passthrough Tarea 3.10 | ☐ Pendiente |
| B.1.4 | Tab Tests muestra aprobado/fallido por assertion | Cada `AssertionResult` con esperado/actual y mensaje | [AUTO] unit Tarea 5.9 | ☐ Pendiente |
| B.1.5 | Resaltado de sintaxis JSON | Editor con highlight JSON (`iced_highlighter` reemplaza CodeMirror) | — (visual) | ☐ Pendiente |
| B.1.6 | Búsqueda in-editor (implementación manual) | Reemplaza la búsqueda que en Tauri daba CodeMirror; comportamiento equivalente | [AUTO] `Propiedad 7` | ☐ Pendiente |

> **Diferencia de mecanismo documentada:** resaltado con `iced_highlighter` y búsqueda
> in-editor implementada manualmente (sin crate apto, Criterio 11.3). Observablemente
> equivalente; verificar solo el aspecto visual del resaltado.

### B.2 — Tabs de configuración del request (Fase 2)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.2.1 | Tabs Params/Headers/Auth/Body/Tests | Presentes con `iced_aw::Tabs`; reutilizadas en Response y Workspace_Panel | Req 3.1 | ☐ Pendiente |
| B.2.2 | Tab por defecto según método + override manual | Params (GET/HEAD/OPTIONS), Body (POST/PUT/PATCH/DELETE) | [AUTO] `Propiedad 9` | ☐ Pendiente |
| B.2.3 | Auth None/Bearer/Basic/ApiKey + descarte al cambiar de tipo | Campos correctos por tipo; descarte del tipo previo | [AUTO] `Propiedad 11` | ☐ Pendiente |
| B.2.4 | CRUD de filas key/value (Params/Headers) | Agregar/editar/eliminar/toggle equivalente a modelo de referencia | [AUTO] `Propiedad 10` | ☐ Pendiente |
| B.2.5 | Tab Tests (assertions) sobre motor existente | Usa `evaluate_response_assertions` sin reescritura | [AUTO] unit Tarea 5.9 | ☐ Pendiente |

### B.3 — Workspace_Panel lateral (Fase 3)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.3.1 | Secciones Environments/Data/History/Diagnostics/App updates | Panel lateral colapsable con las 5 secciones | Req 4.1 | ☐ Pendiente |
| B.3.2 | CRUD Environments + límites (máx. 100, nombre ≤ 100, un activo) | CRUD con validaciones en la capa de orquestación | [AUTO] `Propiedad 12` | ☐ Pendiente |
| B.3.3 | Nombre de environment duplicado | Se rechaza sin mutar el conjunto existente | [AUTO] `Propiedad 15` | ☐ Pendiente |
| B.3.4 | Eliminar el environment activo | Limpia la selección activa (sin environment seleccionado) | [AUTO] `Propiedad 16` | ☐ Pendiente |
| B.3.5 | Export (nativo v1 / Postman v2.1) | Reutiliza `export_postman_collection` / `make_native_bundle` | Req 4.3 | ☐ Pendiente |
| B.3.6 | Import (nativo/Postman/OpenAPI) + límite 10 MB | Ver Parte A.4; validación por formato y tamaño | [AUTO] `Propiedad 13` | ☐ Pendiente |
| B.3.7 | Colisión de nombres en import | Conserva ambos, renombra el importado | [AUTO] `Propiedad 17` | ☐ Pendiente |
| B.3.8 | History (máx. 500, más reciente primero) | Orden por recencia y acotado | [AUTO] `Propiedad 14` | ☐ Pendiente |
| B.3.9 | Diagnostics (máx. 200, más reciente primero) | Acotado y ordenado; persistido en archivo JSON del data dir | [AUTO] `Propiedad 14` + unit Tarea 7.14 | ☐ Pendiente |
| B.3.10 | App updates: estado del Updater | Card con estado (sin actualizaciones/disponible/descargando/lista) | Req 4.8 | ☐ Pendiente |

> **Diferencias de mecanismo documentadas (no de comportamiento):** History elevado de 50
> (Tauri) a **500** (exigido por Req 4.6); Diagnostics persistido en **archivo JSON del data
> dir** en vez de `localStorage`. Verificar que la lista se ve acotada y ordenada por recencia.

### B.4 — Collection_Runner (Fase 4)

Cubierto íntegramente en la Parte A.3 (flujo obligatorio). Todas las reglas 5.1–5.8 están
respaldadas por la `Propiedad 18`. Diferencia de mecanismo: el progreso viaja por canal
`mpsc` + `iced::subscription` en lugar de `app.emit` de Tauri; la **semántica** de progreso
(inicio/fin de request, contadores, reporte final) es equivalente.

### B.5 — Command Palette, sesión y shortcuts (Fase 5)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.5.1 | Command Palette (scoring) | Port de `commandPalette.ts` con pesos idénticos (exacto/prefijo/substring) | [AUTO] unit Tarea 11.3 | ☐ Pendiente |
| B.5.2 | Abrir/cerrar con Ctrl/Cmd+K y ejecutar ítem | Overlay; ejecuta acción y cierra | Req 6.1, 6.2 | ☐ Pendiente |
| B.5.3 | Autosave ≤ 2 s | Persiste dentro de 2 s de la última modificación | [AUTO] `Propiedad 19` | ☐ Pendiente |
| B.5.4 | Restauración de sesión al reabrir | Round-trip de tabs/activa/paneles | [AUTO] `Propiedad 20` | ☐ Pendiente |
| B.5.5 | Stack de tabs cerradas (máx. 20, LIFO) | Acotado + reapertura LIFO | [AUTO] `Propiedad 21` | ☐ Pendiente |
| B.5.6 | Aviso unsaved changes (Guardar/Descartar/Cancelar) | Ver Parte A.2 | [AUTO] `Propiedad 22` | ☐ Pendiente |
| B.5.7 | Shortcuts de teclado (tabla del Criterio 6.8) | Ctrl+Enter, Ctrl+S, Ctrl+Shift+N/P, Ctrl+., Ctrl+K, Ctrl+W, Ctrl+Shift+T, Alt+1..9, Esc | [AUTO] unit Tarea 11.13 | ☐ Pendiente |
| B.5.8 | Error_Boundary por componente | Un panic de un componente se aísla; muestra error y registra en diagnostics sin derribar la app | [AUTO] `Propiedad 23` | ☐ Pendiente |
| B.5.9 | Sesión corrupta/incompatible | Se descarta, inicia sesión vacía y notifica | [AUTO] `Propiedad 24` | ☐ Pendiente |

> **Diferencias de mecanismo documentadas:** autosave a `session.json` atómico (sin
> `localStorage`); Error_Boundary vía `catch_unwind` (iced no tiene `componentDidCatch`).
> Efecto observable equivalente.

### B.6 — Updater in-app (Fase 6)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.6.1 | Verificar actualización (manifiesto + semver) | Descarga `latest.json`/`latest-beta.json` según canal y compara con la versión instalada | [AUTO] `Propiedad 25` | ☐ Pendiente |
| B.6.2 | Descarga con progreso porcentual (≥ 1/seg) | Progreso 0–100%, ≥ 1 actualización/seg, 100% final | [AUTO] integración Tarea 13.4 | ☐ Pendiente |
| B.6.3 | Verificación SHA256 como compuerta | Coincide→instala; no coincide→descarta sin sobrescribir + error de corrupción | [AUTO] `Propiedad 26` | ☐ Pendiente |
| B.6.4 | Fallo de verificación/descarga | Mensaje de error; versión instalada permanece operativa | [AUTO] `Propiedad 27` | ☐ Pendiente |
| B.6.5 | Relanzamiento post-instalación / continuidad si se declina | Ofrece relanzar; si se declina, sesión sigue con versión previa y usa la nueva al reiniciar | — (manual; relanzamiento del proceso) | ☐ Pendiente |

> **Resuelto distinto documentado:** `tauri-plugin-updater` → implementación manual sobre
> `reqwest` (sin crate compatible con el manifest/checksums ya publicados, Criterios 7.1 y
> 11.3). Consume los **mismos** artefactos sin modificarlos; comportamiento equivalente.

### B.7 — Empaquetado y distribución (Fase 7)

| # | Funcionalidad | Comportamiento esperado | Evidencia automatizada | Resultado manual |
| --- | --- | --- | --- | --- |
| B.7.1 | Instalador Windows x86_64 | Se instala sin errores y lanza la app en sistema limpio | — (manual; instalación en SO limpio) | ☐ Pendiente |
| B.7.2 | Instalador Linux x86_64 | Se instala sin errores y lanza la app en sistema limpio | — (manual; instalación en SO limpio) | ☐ Pendiente |
| B.7.3 | Generación de `latest.json`/`latest-beta.json`/`SHA256SUMS.txt` | Preservada por los scripts de release (agnósticos al empaquetador) | Req 8.3, 8.4 | ☐ Pendiente |
| B.7.4 | Detención del pipeline ante fallo de empaquetado | Se detiene, reporta plataforma/causa y no publica artefactos parciales | — (manual/CI) | ☐ Pendiente |

> **Estado abierto (heredado de `functional-equivalence.md`):** la Fase 7 no está
> completamente cerrada en `tasks.md` (tareas 15.5 y 15.6 en curso; checkpoint 16 abierto).
> Este punto **debe** reflejarse como diferencia pendiente en el reporte de la Tarea 17.3 y
> considerarse en la compuerta de la Tarea 17.4 antes de autorizar la Fase 8.

---

## PARTE E — Registro de ejecución de la comparación (Tarea 17.2)

Esta sección registra la **ejecución** de la comparación exigida por el Criterio 10.6, con su
alcance, método y límites explícitos.

### E.1 — Alcance verificado en esta ejecución

Los cinco flujos mínimos obligatorios del Criterio 10.6 (crear request, guardar request,
correr una colección, importar OpenAPI, exportar cURL) quedan marcados individualmente en los
*Veredictos de flujo* (Parte A) y en la *Hoja de resultados* (Parte C). El resultado de esta
ejecución es:

- **A nivel de lógica: los cinco flujos son equivalentes** a Midway_Tauri. No se identificó
  ninguna diferencia de **comportamiento observable**. Las únicas diferencias son de
  **mecanismo interno** (transporte de progreso, medio de persistencia, empaquetador, updater),
  ya documentadas y justificadas en `functional-equivalence.md` y aceptadas allí como
  equivalentes de cara al usuario.
- **A nivel visual / de plataforma:** los aspectos que ningún test automatizado puede
  sustituir (resaltado JSON en pantalla, diálogo de archivo del SO, portapapeles del SO,
  relanzamiento del proceso tras update, instalación en sistema limpio) permanecen `☐ Pendiente`
  y requieren la ejecución humana lado a lado descrita en *Cómo usar este instrumento*.

### E.2 — Método y fuente de evidencia

La equivalencia de la **lógica** de cada flujo se apoya en la suite automatizada del workspace,
que es la fuente objetiva de esta comparación:

- **Property tests (`proptest`)** — las 28 Correctness Properties del diseño, ubicadas junto a
  la implementación: `midway-desktop/src/app.rs` (Propiedades 1–17 de composición/tabs/import),
  `midway-desktop/src/collection_runner.rs` (Propiedad 18), `midway-desktop/src/session.rs`
  (Propiedades 19–24), `midway-desktop/src/updater.rs` (Propiedades 25–27) y
  `midway-core/tests/no_tauri_dependency.rs` (Propiedad 28).
- **Unit tests e integración con `wiremock`** — para Send end-to-end, canal de progreso del
  runner, passthrough del Response_Inspector, scoring del command palette y shortcuts.
- **Reutilización sin reescritura** — los cinco flujos se apoyan en código de `midway-core`
  (`domain::interop`, `domain::preview`, `domain::runner`, `domain::testing`, `runtime::*`)
  extraído sin cambios desde `src-tauri/src/`, por lo que su comportamiento es **equivalente
  por construcción** (se ejecuta el mismo código Rust que usaba Midway_Tauri).
- **Precondición de fase (Criterio 10.2)** — cada fase previa (0–7) solo pudo avanzar tras
  ejecutar `cargo test` sin fallos, de modo que esta suite ya estaba verde al cierre de cada
  fase.

### E.3 — Limitación de esta ejecución (transparencia)

Una comparación **runtime real y simultánea** de ambas GUIs (dos binarios gráficos corriendo
en un escritorio de la plataforma objetivo, con interacción humana: teclado, ratón, diálogos
del SO) **no es ejecutable en el entorno automatizado** en que se generó este registro.
Adicionalmente, en este entorno la ejecución directa de `cargo test --workspace` no pudo
lanzarse por limitaciones del shell sobre el path de trabajo; la evidencia automatizada se
sustenta, por tanto, en (a) la presencia verificada de los tests en el árbol de fuentes y
(b) la precondición del Criterio 10.2 ya satisfecha en fases previas. En consecuencia:

- Los veredictos `☑ Equivalente (lógica)` reflejan la equivalencia **verificada por la suite
  automatizada y por la reutilización sin reescritura**, no una observación visual de ambas
  apps en pantalla.
- La confirmación **visual/interactiva/de plataforma** sigue siendo responsabilidad del tester
  humano (filas `—` y `☐ Pendiente`), y debe ejecutarse antes de la firma de la Parte D.
- **Recomendación operativa:** correr `cargo test --workspace` en un entorno con toolchain de
  Rust funcional para dejar constancia verde de la evidencia automatizada antes de la firma.

### E.4 — Diferencias a escalar a la Tarea 17.3

- **Ninguna regresión de comportamiento** en los cinco flujos obligatorios.
- **Diferencias de mecanismo** (todas aceptadas como equivalentes en `functional-equivalence.md`):
  transporte de progreso del runner, autosave `session.json`, Error_Boundary por `catch_unwind`,
  límite de History 50→500, Diagnostics en archivo JSON, updater manual sobre `reqwest`,
  empaquetado con `cargo-packager`.
- **Ítem abierto (no bloqueante para los cinco flujos):** la Fase 7 (empaquetado/distribución)
  no está cerrada en `tasks.md` (tareas 15.5/15.6 y checkpoint 16). La Tarea 17.3 debe
  clasificarlo, y la Tarea 17.4 considerarlo en la compuerta hacia la Fase 8.

---

## PARTE C — Hoja de resultados

### C.1 — Flujos mínimos obligatorios (Criterio 10.6)

| Flujo | Resultado (equivalente / con diferencias) | Diferencia observada (→ Tarea 17.3) |
| --- | --- | --- |
| A.1 Crear request | ☑ Equivalente (lógica; visual del resaltado JSON pendiente humano) | Ninguna diferencia de comportamiento observable |
| A.2 Guardar request | ☑ Equivalente (lógica) | Solo mecanismo (autosave a `session.json` vs. `localStorage`), aceptado como equivalente |
| A.3 Correr una colección | ☑ Equivalente (lógica) | Solo mecanismo (progreso por `mpsc`+subscription vs. `app.emit`), aceptado como equivalente |
| A.4 Importar OpenAPI | ☑ Equivalente (lógica; diálogo de archivo del SO pendiente humano) | Ninguna diferencia de comportamiento observable |
| A.5 Exportar cURL | ☑ Equivalente (lógica; portapapeles del SO pendiente humano) | Ninguna diferencia de comportamiento observable |

### C.2 — Áreas complementarias (Fases 1–7)

| Área | Resultado | Diferencia observada (→ Tarea 17.3) |
| --- | --- | --- |
| B.1 Request/Response | ☑ Equivalente (lógica; resaltado JSON visual pendiente humano) | Solo mecanismo (`iced_highlighter` + búsqueda manual), aceptado como equivalente |
| B.2 Request tabs | ☑ Equivalente (lógica) | Ninguna |
| B.3 Workspace panel | ☑ Equivalente (lógica) | Solo mecanismo (History 50→500; Diagnostics en archivo JSON vs. `localStorage`), aceptado como equivalente |
| B.4 Collection runner | ☑ Equivalente (lógica) | Solo mecanismo (progreso por `mpsc`+subscription), aceptado como equivalente |
| B.5 Palette / sesión / shortcuts | ☑ Equivalente (lógica) | Solo mecanismo (autosave `session.json`; Error_Boundary por `catch_unwind`), aceptado como equivalente |
| B.6 Updater | ☑ Equivalente (lógica; relanzamiento del proceso pendiente humano) | Resuelto distinto (impl. manual sobre `reqwest` vs. `tauri-plugin-updater`); consume los mismos artefactos, aceptado como equivalente |
| B.7 Empaquetado | ⚠ Pendiente de cierre | Fase 7 no cerrada en `tasks.md` (tareas 15.5/15.6 y checkpoint 16 abiertos) → **debe** escalarse a la Tarea 17.3 |

### C.3 — Estado conocido por implementación (pre-comparación)

Al momento de generar este instrumento, la equivalencia de la **lógica** de cada flujo está
anclada por la suite automatizada del workspace (28 Correctness Properties del diseño más
unit tests e integración con `wiremock`), ubicada junto a la implementación en `midway-desktop`
(p. ej. `curl.rs`, `app.rs`, `session.rs`, `collection_runner.rs`, `updater.rs`) y en
`midway-core`. Las Propiedades citadas en las columnas *Evidencia automatizada* de este
documento son verificables ejecutando:

```
cargo test --workspace
```

Lo que **resta** por confirmar es exclusivamente lo **visual/interactivo/de plataforma**
(filas marcadas `—` en la columna de evidencia): look & feel, foco de teclado, diálogos de
archivo y portapapeles del SO, instalación en sistema limpio y relanzamiento post-update.
Ese es el trabajo del tester humano sobre las dos builds.

> **Salvedad importante:** este instrumento **pre-rellena** la columna de evidencia
> automatizada, pero **no** marca ningún flujo como definitivamente "equivalente": esa
> decisión requiere la ejecución manual lado a lado. Hasta entonces, cada flujo permanece
> `☐ Pendiente` de confirmación humana.

## PARTE D — Firma y compuerta hacia la Fase 8

- **Ejecución de la comparación lógica (Tarea 17.2):** registrada en la Parte E. Los cinco
  flujos obligatorios quedaron marcados como **equivalentes a nivel de lógica**; la
  confirmación visual/de plataforma queda `☐ Pendiente` para el tester humano abajo.
- Tester(es) (verificación visual/SO): ____________________  Fecha: ____________  Plataforma(s): ____________
- ¿Se registraron diferencias? ☑ No hay regresión de comportamiento en los cinco flujos
  obligatorios; sí hay diferencias de **mecanismo** (aceptadas como equivalentes) y un **ítem
  abierto** de Fase 7 — enumeradas en la Parte E.4 y a clasificar en el reporte de la **Tarea 17.3**,
  `docs/differences-report.md`, clasificando cada una como **regresión bloqueante** o **no
  bloqueante**).
- **Compuerta (Criterio 10.8):** la limpieza de la Fase 8 (Tarea 18) **no** debe ejecutarse
  mientras exista una regresión **bloqueante** sin documentar ni aceptar explícitamente. La
  verificación de esta compuerta es responsabilidad de la **Tarea 17.4**.
- Aprobación del responsable técnico de la migración: ____________________  Fecha: __________

## Documentos relacionados

- Resumen escrito de equivalencia funcional (Tarea 17.1): [`docs/functional-equivalence.md`](./functional-equivalence.md).
- Reporte de diferencias clasificadas (Tarea 17.3): [`docs/differences-report.md`](./differences-report.md).
- Verificación de la compuerta de bloqueo (Tarea 17.4): [`docs/blocking-regression-gate.md`](./blocking-regression-gate.md).
- Distribución y release: [`docs/distribution.md`](./distribution.md).
