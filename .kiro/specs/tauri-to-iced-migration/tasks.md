# Implementation Plan: Migración Tauri → iced (Midway Desktop)

## Overview

Este plan implementa la migración de Midway desde Tauri + React/TypeScript a una aplicación 100% Rust sobre `iced`, en 8 fases secuenciales (Fase 0 a Fase 8) más un requisito transversal de verificación por fase (Requisito 10) y restricciones técnicas transversales (Requisito 11). Cada fase termina en un checkpoint que exige `cargo check` y `cargo test` exitosos sobre el workspace completo antes de avanzar (Requisito 10, Criterios 1-3), preservando `midway` (Tauri) funcional hasta la Fase 8. Las 28 Correctness Properties del diseño se implementan como tests de propiedad con `proptest`, ubicados junto a la implementación que verifican, y anotados con su número de propiedad y el/los criterios de requisitos que validan.

## Tasks

- [x] 1. Fase 0: Reestructuración como Cargo workspace
  - [x] 1.1 Crear Cargo workspace raíz con members `midway-core`, `midway-desktop` y `midway` (`src-tauri`)
    - Declarar `[workspace] members = ["midway-core", "midway-desktop", "src-tauri"]` en el `Cargo.toml` raíz
    - _Design: Architecture > Cargo workspace_
    - _Requirements: 1.1_

  - [x] 1.2 Crear crate `midway-core` moviendo `domain/`, `infra/` y `runtime/` desde `src-tauri/src/` sin reescritura de lógica
    - Ajustar únicamente rutas de módulos y visibilidad `pub` necesaria para consumo externo desde `midway-desktop` y `midway`
    - _Design: Architecture > Cargo workspace (estructura de directorios); Components and Interfaces_
    - _Requirements: 1.2, 1.3, 1.9_

  - [x] 1.3 Eliminar el acoplamiento a Tauri dentro de `midway-core`
    - Reemplazar `tauri::async_runtime::spawn` por `tokio::spawn` en `runtime/secret_executor.rs` sin cambiar el comportamiento observable del executor
    - Verificar que `midway-core` no declare ninguna dependencia directa de `tauri` en su `Cargo.toml`
    - _Design: Overview (único punto de acoplamiento a Tauri); Integración async con midway-core_
    - _Requirements: 1.10_

  - [x] 1.4 Escribir property test para independencia de `midway-core` respecto a Tauri
    - **Property 28: `midway-core` no depende de `tauri`**
    - **Validates: Requirements 1.10**
    - Implementar como test determinístico único (no aleatorio) que parsea `cargo metadata --format-version 1` filtrado al subgrafo de `midway-core` y falla si aparece `tauri` o cualquier paquete `tauri-*`

  - [x] 1.5 Adaptar el crate `midway` (Tauri) para depender de `midway-core`
    - `commands/mod.rs` y `state.rs` permanecen Tauri-específicos pero delegan su lógica de orquestación a las funciones públicas de `midway-core`, sin cambiar el comportamiento observable de ningún comando existente
    - _Design: Overview (commands/mod.rs y state.rs no se migran tal cual)_
    - _Requirements: 1.6, 1.7, 1.9_

  - [x] 1.6 Crear el crate binario `midway-desktop` con esqueleto mínimo dependiente de `midway-core` e `iced`
    - Fijar en `Cargo.toml` una versión exacta de `iced` (sin `^`, `~` ni `*`), verificable de forma idéntica en `Cargo.toml` y `Cargo.lock`
    - _Design: Architecture > Cargo workspace; Elm architecture en iced_
    - _Requirements: 1.4, 1.5, 11.1_

  - [x] 1.7 Escribir tests de verificación de compilación y no-regresión del workspace
    - Verificar que `cargo check` compila sin errores los tres crates miembros
    - Verificar, para las firmas públicas modificadas durante la extracción, que la suite de tests existente del crate `midway` sigue pasando sin regresiones
    - _Requirements: 1.8, 1.9_

- [x] 2. Checkpoint - Fase 0
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo (los tres crates); bloquear el avance a la Fase 1 si alguno falla. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 3. Fase 1: Request composer y response inspector
  - [x] 3.1 Definir el esqueleto de la aplicación iced (`app.rs`, `state.rs`, `main.rs`)
    - Definir `Message` (enum agrupado por área), struct `Midway` (State) y arrancar `iced::application(...).subscription(...).run_with(...)` con `AppState` (repositorio, request_executor, secret_executor)
    - _Design: Data Models > Mensajes de nivel superior; Elm architecture en iced_
    - _Requirements: 1.4_

  - [x] 3.2 Implementar la fila superior del Request_Composer
    - `pick_list` de método HTTP (GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS), `text_input` de URL, botón Send, `pick_list` de environment y botón de settings (ícono engranaje)
    - _Design: Components and Interfaces > Request_Composer y Response_Inspector_
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [x] 3.3 Implementar `Curl_Importer` (`curl.rs`): tokenizador y extracción de campos
    - Port de `src/lib/curl.ts`: manejo de comillas simples/dobles/escapes, mapeo de flags `-X/--request/-H/--header/-u/--user/-d/--data*/-F/--form*/--json/-G/--get/-I/--head`, inferencia de body JSON/texto/form-data
    - _Design: Components and Interfaces > Curl_Importer_
    - _Requirements: 2.6, 2.16_

  - [x] 3.4 Escribir property test de equivalencia de parseo cURL con la referencia TypeScript
    - **Property 1: Equivalencia de parseo cURL con la referencia TypeScript**
    - **Validates: Requirements 2.6, 2.16**

  - [x] 3.5 Implementar el enrutamiento de pegado de cURL según el estado de la tab activa
    - Detectar `looks_like_curl_command`, decidir sobrescribir la tab activa (vacía) o crear una nueva tab (no vacía); mostrar error sin modificar contenido si el parseo falla; insertar como texto plano si no parece cURL
    - _Design: Components and Interfaces > Curl_Importer_
    - _Requirements: 2.7, 2.8, 2.9, 2.19_

  - [x] 3.6 Escribir property test de enrutamiento de pegado de cURL según estado de la tab
    - **Property 2: Enrutamiento de pegado de cURL según estado de la tab**
    - **Validates: Requirements 2.7, 2.8**

  - [x] 3.7 Escribir property test de inserción de texto no-cURL como texto plano
    - **Property 3: Texto no-cURL se inserta como texto plano**
    - **Validates: Requirements 2.9**

  - [x] 3.8 Escribir property test de cURL malformado sin mutación del contenido existente
    - **Property 4: cURL malformado no muta el contenido existente**
    - **Validates: Requirements 2.19**

  - [x] 3.9 Implementar el Response_Inspector
    - Mostrar status, tiempo de respuesta (ms) y tamaño (bytes) al finalizar una petición; tabs Body/Headers/Tests, donde la tab Tests muestra el resultado (aprobado/fallido) de cada assertion evaluada
    - _Design: Components and Interfaces > Request_Composer y Response_Inspector; Flujo de una request (Fase 1)_
    - _Requirements: 2.10, 2.11_

  - [x] 3.10 Escribir unit tests de passthrough del Response_Inspector
    - Cubrir status/tiempo/tamaño y resultados de assertions renderizados sin transformación adicional
    - _Requirements: 2.10, 2.11_

  - [x] 3.11 Implementar `Text_Editor_Component` (`ui/text_editor.rs`)
    - Wrapper sobre `iced::widget::text_editor::Content` + `iced_highlighter::Highlighter` con resaltado de sintaxis JSON, usado para body de request, body de response y preview
    - _Design: Components and Interfaces > Text_Editor_Component_
    - _Requirements: 2.12_

  - [x] 3.12 Implementar el formateo de contenido JSON en `Text_Editor_Component`
    - Usar `serde_json::from_str` + `serde_json::to_string_pretty` al activar la acción de formateo
    - _Design: Components and Interfaces > Text_Editor_Component_
    - _Requirements: 2.13_

  - [x] 3.13 Escribir property test de idempotencia y preservación de valor del formateo JSON
    - **Property 5: Formateo JSON es idempotente y preserva el valor**
    - **Validates: Requirements 2.13**

  - [x] 3.14 Implementar el lint de JSON inválido en `Text_Editor_Component`
    - Capturar `serde_json::Error`, extraer `.line()`/`.column()` y mostrar el error inline sin descartar el contenido existente
    - _Design: Components and Interfaces > Text_Editor_Component; Error Handling_
    - _Requirements: 2.14_

  - [x] 3.15 Escribir property test de posición exacta del error de lint JSON
    - **Property 6: El lint JSON reporta la posición exacta del error**
    - **Validates: Requirements 2.14**

  - [x] 3.16 Implementar la búsqueda de texto en `Text_Editor_Component`
    - Cálculo manual de coincidencias case-insensitive sobre el texto plano del `Content`, navegación secuencial siguiente/anterior con resaltado de rango
    - _Design: Components and Interfaces > Text_Editor_Component_
    - _Requirements: 2.15_

  - [x] 3.17 Escribir property test de cobertura completa de búsqueda de texto
    - **Property 7: La búsqueda de texto encuentra todas las coincidencias sin omitir ni duplicar**
    - **Validates: Requirements 2.15**

  - [x] 3.18 Implementar la ejecución de Send mediante llamadas directas a `midway-core`
    - `Message::RequestComposer(SendPressed)` invoca `resolve_request` (interpolation + auth) y `request_executor.execute` vía `Task::perform`, actualizando el `Response_Inspector` con el resultado
    - _Design: Flujo de una request (Fase 1); Integración async con midway-core_
    - _Requirements: 2.17_

  - [x] 3.19 Escribir test de integración del flujo Send end-to-end
    - Servidor HTTP mock local (`wiremock` o `mockito`, versión pinneada) verificando que Send invoca `resolve_request` + `execute` con el draft de la tab activa y que la respuesta llega al `Response_Inspector`
    - _Requirements: 2.17_

  - [x] 3.20 Implementar el preview drawer del control de settings del request
    - Al activar settings, mostrar `domain::preview::make_preview(resolution)` (método, URL final, headers, body) y el comando cURL equivalente de `domain::preview::make_curl_command`
    - _Design: Components and Interfaces > Preview_
    - _Requirements: 2.18_

  - [x] 3.21 Escribir property test de fidelidad del preview respecto al draft vigente
    - **Property 8: El preview refleja exactamente el draft vigente**
    - **Validates: Requirements 2.18**

- [x] 4. Checkpoint - Fase 1
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 5. Fase 2: Tabs de configuración del request
  - [x] 5.1 Implementar el contenedor de tabs Params/Headers/Auth/Body/Tests con `iced_aw::{TabBar, Tabs}`
    - Reutilizar el mismo widget para las tabs de Response (Body/Headers/Tests) y para las secciones del Workspace_Panel
    - _Design: Components and Interfaces > Tabs de configuración del request (Fase 2)_
    - _Requirements: 3.1_

  - [x] 5.2 Implementar la selección de tab por defecto según método HTTP con soporte de override manual
    - Calcular en `update`, al cambiar el método, Params por defecto para GET/HEAD/OPTIONS y Body por defecto para POST/PUT/PATCH/DELETE, respetando el flag `manual_tab_override` en `RequestTabState`
    - _Design: Components and Interfaces > Tabs de configuración del request (Fase 2)_
    - _Requirements: 3.2, 3.3_

  - [x] 5.3 Escribir property test de selección de tab por defecto según método HTTP
    - **Property 9: Selección de tab por defecto según método HTTP, salvo override manual**
    - **Validates: Requirements 3.2, 3.3**

  - [x] 5.4 Implementar la tab Auth (None/Bearer/Basic/ApiKey)
    - `pick_list` de tipo sobre `domain::http::AuthConfig`; Bearer muestra campo token; Basic muestra usuario/contraseña; ApiKey muestra nombre de clave, valor y ubicación (header o query param); al cambiar de tipo se reemplaza la variante completa por su default vacío
    - _Design: Components and Interfaces > Tabs de configuración del request (Fase 2)_
    - _Requirements: 3.4, 3.5, 3.6, 3.7, 3.10_

  - [x] 5.5 Escribir property test de descarte de campos al cambiar el tipo de autenticación
    - **Property 11: Cambiar el tipo de autenticación descarta los campos anteriores**
    - **Validates: Requirements 3.10**

  - [x] 5.6 Implementar el editor de filas key/value para las tabs Params y Headers
    - Reutilizar `domain::http::KeyValueRow`; acciones agregar, editar, eliminar y alternar `enabled`
    - _Design: Components and Interfaces > Tabs de configuración del request (Fase 2)_
    - _Requirements: 3.8_

  - [x] 5.7 Escribir property test de equivalencia de CRUD de filas key/value contra un modelo de referencia
    - **Property 10: CRUD de filas key/value equivale a un modelo de referencia**
    - **Validates: Requirements 3.8**

  - [x] 5.8 Implementar la tab Tests (editor de assertions)
    - Editor de `domain::testing::ResponseAssertion` (source: Status/Header/BodyText/JsonPointer/FinalUrl; operator: Equals/Contains/NotContains/Exists/NotExists/GreaterOrEqual/LessOrEqual; selector opcional; expected), invocando `evaluate_response_assertions` (motor existente, sin reescritura) al recibir una respuesta
    - _Design: Components and Interfaces > Tabs de configuración del request (Fase 2)_
    - _Requirements: 3.9_

  - [x] 5.9 Escribir unit tests de la tab Tests sobre el motor de assertions existente
    - Cubrir al menos un caso por cada combinación source/operator representativa, verificando que no se reescribe la lógica de `domain/testing.rs`
    - _Requirements: 3.9_

- [x] 6. Checkpoint - Fase 2
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 7. Fase 3: Workspace panel lateral
  - [x] 7.1 Implementar el contenedor del Workspace_Panel con secciones Environments/Data/History/Diagnostics/App updates
    - Panel lateral colapsable usando `iced_aw::Tabs`
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3)_
    - _Requirements: 4.1_

  - [x] 7.2 Implementar CRUD de Environments con validación de duplicados y límites
    - CRUD sobre `domain::workspace::EnvironmentRecord` vía `infra::sqlite_repository::SqliteRepository`; máximo un environment activo a la vez; validación de duplicado de nombre, máximo 100 environments y máximo 100 caracteres por nombre en la capa de orquestación de `midway-desktop` antes de invocar `save_environment`
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3); Error Handling_
    - _Requirements: 4.2, 4.9, 4.10_

  - [x] 7.3 Escribir property test de invariantes de CRUD de environments
    - **Property 12: Invariantes de CRUD de environments**
    - **Validates: Requirements 4.2**

  - [x] 7.4 Escribir property test de rechazo de nombre de environment duplicado
    - **Property 15: Nombre de environment duplicado se rechaza sin mutar el conjunto existente**
    - **Validates: Requirements 4.9**

  - [x] 7.5 Escribir property test de limpieza de selección al eliminar el environment activo
    - **Property 16: Eliminar el environment activo limpia la selección activa**
    - **Validates: Requirements 4.10**

  - [x] 7.6 Implementar export de datos en la sección Data (workspace v1 nativo y Postman Collection v2.1)
    - Reutilizar `domain::interop::export_postman_collection` y `make_native_bundle` sin reescritura
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3)_
    - _Requirements: 4.3_

  - [x] 7.7 Implementar import de datos en la sección Data (nativo v1, Postman v2.1, OpenAPI v3 JSON/YAML) con límite de 10 MB
    - Reutilizar `domain::interop::import_postman_collection`, `import_openapi_document`, `parse_native_bundle` sin reescritura; validar el tamaño del payload (10 MB) antes de invocar el parser; mostrar error descriptivo sin mutar el estado existente ante formato no soportado, contenido corrupto o tamaño excedido
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3); Error Handling_
    - _Requirements: 4.4, 4.5_

  - [x] 7.8 Escribir property test de validación de import por formato y tamaño
    - **Property 13: Validación de import por formato y tamaño**
    - **Validates: Requirements 4.4, 4.5**

  - [x] 7.9 Implementar la resolución de colisión de nombres al importar
    - Al importar un environment, request o colección con nombre ya existente, conservar ambos elementos asignando un nombre diferenciado al elemento importado
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3)_
    - _Requirements: 4.11_

  - [x] 7.10 Escribir property test de preservación de ambos elementos ante colisión de nombre en import
    - **Property 17: Colisión de nombre en import preserva ambos elementos**
    - **Validates: Requirements 4.11**

  - [x] 7.11 Implementar la sección History con límite de 500 entradas
    - Listar `domain::workspace::HistoryEntry` desde `workspace_snapshot`, orden de más reciente a más antigua; introducir constante `HISTORY_LIMIT = 500` en `midway-desktop` (ajustando el límite de 50 usado hoy en `commands/mod.rs`)
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3)_
    - _Requirements: 4.6_

  - [x] 7.12 Implementar el módulo `diagnostics.rs` y la sección Diagnostics con límite de 200 entradas
    - Port de `src/lib/diagnostics.ts`: persistir hasta 200 `CrashRecord` en un archivo JSON en el data dir de la app; listar del más reciente al más antiguo
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3); Data Models > Formato de diagnostics_
    - _Requirements: 4.7_

  - [x] 7.13 Escribir property test de colecciones acotadas ordenadas por recencia (History y Diagnostics)
    - **Property 14: Colecciones acotadas ordenadas de la más reciente a la más antigua**
    - **Validates: Requirements 4.6, 4.7**

  - [x] 7.14 Escribir unit tests de camino nominal y borde para `diagnostics.rs`
    - Cubrir un caso de camino nominal (registro típico) y un caso borde (registro en el límite de 200 entradas)
    - _Requirements: 10.4_

  - [x] 7.15 Implementar la sección App updates (estado del Updater sin lógica de red aún)
    - Card con el estado actual del Updater (sin actualizaciones/disponible/descargando/lista para instalar), reemplazo directo de `UpdateCenterCard.tsx`; la lógica de red se implementa en la Fase 6
    - _Design: Components and Interfaces > Workspace_Panel lateral (Fase 3)_
    - _Requirements: 4.8_

- [x] 8. Checkpoint - Fase 3
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 9. Fase 4: Collection runner
  - [x] 9.1 Implementar `collection_runner.rs` con ejecución secuencial de requests
    - Función libre `run_collection(...)` migrada desde el comando `run_collection` de `commands/mod.rs`, reutilizando `domain::runner` (`CollectionRunReport`, `CollectionRunItem`, fases) sin reescritura; ejecutar los requests en el orden en que están guardados
    - _Design: Components and Interfaces > Collection_Runner (Fase 4)_
    - _Requirements: 5.1_

  - [x] 9.2 Implementar el reporte de progreso incremental vía canal `mpsc` y `iced::subscription`
    - Sustituir `app.emit(COLLECTION_RUN_PROGRESS_EVENT, ...)` por mensajes de progreso (inicio/fin de request, contadores) reenviados a `update` mientras `CollectionRunnerState::running` sea `Some`
    - _Design: Components and Interfaces > Collection_Runner (Fase 4)_
    - _Requirements: 5.2_

  - [x] 9.3 Implementar el reporte consolidado final y el uso de environment de override
    - Reporte con indicador de éxito/fallo, status HTTP, tiempo de respuesta y resultados de assertions por request, más totales de éxito/fallo de la colección; usar el environment de override cuando esté definido o el activo por defecto en caso contrario
    - _Design: Components and Interfaces > Collection_Runner (Fase 4)_
    - _Requirements: 5.3, 5.4, 5.6_

  - [x] 9.4 Implementar el manejo de fallos por request (red/timeout/assertion) sin detener la secuencia
    - Registrar el fallo y su motivo en el reporte consolidado y continuar con el siguiente request
    - _Design: Error Handling_
    - _Requirements: 5.5_

  - [x] 9.5 Implementar el caso de colección vacía (N=0)
    - Presentar de inmediato un reporte consolidado con cero requests ejecutados, sin realizar ninguna llamada HTTP
    - _Design: Components and Interfaces > Collection_Runner (Fase 4)_
    - _Requirements: 5.7_

  - [x] 9.6 Implementar la cancelación de una ejecución en curso
    - `oneshot::Sender<()>` compartido entre la tarea de ejecución y el botón Cancelar, replicando el patrón de `RequestExecutorHandle::cancel`; el reporte consolidado refleja únicamente los requests completados hasta la cancelación
    - _Design: Components and Interfaces > Collection_Runner (Fase 4)_
    - _Requirements: 5.8_

  - [x] 9.7 Escribir property test de ejecución secuencial y reporte consolidado del Collection_Runner
    - **Property 18: Ejecución secuencial y reporte consolidado del Collection_Runner**
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8**
    - Usar un mock de `RequestExecutorHandle` inyectado (sin llamadas HTTP reales) parametrizado por N requests, resultados simulados por request y punto de cancelación k

- [x] 10. Checkpoint - Fase 4
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 11. Fase 5: Command palette, persistencia de sesión y shortcuts
  - [x] 11.1 Implementar `command_palette.rs` (port del algoritmo de scoring)
    - Port directo de `src/lib/commandPalette.ts`: normalización, coincidencia exacta/prefijo/substring sobre título, keywords y subtítulo, con pesos idénticos, operando sobre `CommandPaletteItem` (acciones fijas, colecciones, requests guardados)
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.1_

  - [x] 11.2 Implementar la apertura/cierre del Command_Palette y la ejecución de ítems
    - Abrir con Ctrl/Cmd+K (overlay vía `iced::widget::stack`); al seleccionar una acción/colección/request, ejecutar la acción correspondiente y cerrar el palette
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.1, 6.2_

  - [x] 11.3 Escribir unit tests de apertura del Command_Palette y ejecución de ítems
    - Cubrir un caso de camino nominal (búsqueda con coincidencia) y un caso borde (búsqueda sin coincidencias)
    - _Requirements: 10.4_

  - [x] 11.4 Implementar `session.rs`: `SessionSnapshot`, escritura atómica y autosave
    - Serializar `SessionSnapshot { version, open_tabs, active_tab_id, panel_sizes, closed_tabs }` a `session.json` mediante escritura a archivo temporal + rename; disparar el guardado desde una subscription `iced::time::every`, persistiendo solo si hubo cambios desde el último guardado
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5); Data Models > Formato de sesión persistida_
    - _Requirements: 6.3_

  - [x] 11.5 Escribir property test de autosave dentro de los 2 segundos desde la última modificación
    - **Property 19: El autosave persiste dentro de 2 segundos desde la última modificación**
    - **Validates: Requirements 6.3**
    - Usar un reloj simulado (fake clock) para el timer de autosave

  - [x] 11.6 Implementar la restauración de sesión al reabrir la aplicación
    - Restaurar tabs abiertas, tab activa y paneles redimensionados a partir del último estado guardado por autosave
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.4_

  - [x] 11.7 Escribir property test de round-trip de la sesión persistida
    - **Property 20: Round-trip de la sesión persistida**
    - **Validates: Requirements 6.4, 6.7**

  - [x] 11.8 Implementar el stack de tabs cerradas (`VecDeque<TabSnapshot>` acotado a 20, reapertura LIFO)
    - Descartar la entrada más antigua al superar 20 elementos; reabrir extrae la tab cerrada más recientemente
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.5_

  - [x] 11.9 Escribir property test del stack acotado de tabs cerradas con reapertura LIFO
    - **Property 21: El stack de tabs cerradas está acotado y reabre en orden LIFO**
    - **Validates: Requirements 6.5**

  - [x] 11.10 Implementar el aviso de unsaved changes al cerrar una tab con cambios sin guardar
    - Modal overlay con opciones Guardar (persiste y cierra), Descartar (cierra sin persistir) y Cancelar (mantiene la tab abierta sin cambios)
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.6_

  - [x] 11.11 Escribir property test de las acciones del aviso de unsaved changes
    - **Property 22: Las acciones del aviso de unsaved changes son correctas**
    - **Validates: Requirements 6.6**

  - [x] 11.12 Implementar los shortcuts de teclado globales
    - `iced::keyboard::on_key_press` a nivel de aplicación mapeado a: Ctrl+Enter (Send), Ctrl+S (Guardar), Ctrl+Shift+N (Nuevo request), Ctrl+Shift+P (Preview), Ctrl+. (Tools/Workspace), Ctrl+K (Command Palette), Ctrl+W (Cerrar tab activa), Ctrl+Shift+T (Reabrir tab), Alt+1..9 (ir a tab N) y Esc (cerrar panel/settings)
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5)_
    - _Requirements: 6.8_

  - [x] 11.13 Escribir unit tests de la tabla fija de shortcuts
    - Verificar, para cada combinación de teclas del Criterio 6.8, que se despacha el mensaje/acción correspondiente
    - _Requirements: 6.8_

  - [x] 11.14 Implementar el Error_Boundary por componente
    - Envolver cada handler de área de `update` y cada cierre de `view` de un componente aislado en `std::panic::catch_unwind` (con `AssertUnwindSafe` donde corresponda); al capturar un panic, registrar un `CrashRecord` con `source = ComponentBoundary` vía `diagnostics::append_crash_record` y sustituir el `Element` del componente por un mensaje de error visible, sin interrumpir el resto de la aplicación
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5); Error Handling_
    - _Requirements: 6.9_

  - [x] 11.15 Escribir property test de aislamiento de panics por el Error_Boundary
    - **Property 23: El Error_Boundary aísla panics de un componente sin derribar la aplicación**
    - **Validates: Requirements 6.9**

  - [x] 11.16 Implementar el descarte seguro de sesión corrupta o incompatible
    - Al leer `session.json`, si falla la deserialización o `version` no es compatible, descartar el contenido, iniciar una sesión vacía y notificar al usuario, sin abortar el arranque
    - _Design: Components and Interfaces > Command Palette, Session_Store y shortcuts (Fase 5); Error Handling_
    - _Requirements: 6.10_

  - [x] 11.17 Escribir property test de descarte seguro de sesión corrupta o incompatible
    - **Property 24: Sesión corrupta o incompatible se descarta de forma segura**
    - **Validates: Requirements 6.10**

- [x] 12. Checkpoint - Fase 5
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 13. Fase 6: Updater in-app
  - [x] 13.1 Implementar `updater.rs`: descarga de manifiesto y comparación semver
    - Descargar `latest.json`/`latest-beta.json` según canal configurado; comparar con `env!("CARGO_PKG_VERSION")` usando el crate `semver` (versión pinneada); informar si hay una versión más reciente disponible
    - _Design: Components and Interfaces > Updater in-app (Fase 6); Data Models > Manifest del updater_
    - _Requirements: 7.1, 7.2, 11.1_

  - [x] 13.2 Escribir property test de comparación semver para disponibilidad de actualización
    - **Property 25: La comparación semver determina correctamente la disponibilidad de actualización**
    - **Validates: Requirements 7.2**

  - [x] 13.3 Implementar la descarga del artefacto con reporte de progreso porcentual
    - Descargar vía `reqwest::Response::bytes_stream`, reportando progreso entre 0% y 100% con frecuencia mínima de 1 actualización por segundo
    - _Design: Components and Interfaces > Updater in-app (Fase 6)_
    - _Requirements: 7.3_

  - [x] 13.4 Escribir test de integración de progreso de descarga del Updater
    - Servidor HTTP local que sirve un artefacto en chunks controlados, verificando frecuencia mínima de 1/seg y porcentaje final de 100%
    - _Requirements: 7.3_

  - [x] 13.5 Implementar la verificación de checksum SHA256 como compuerta antes de instalar
    - Calcular SHA256 (crate `sha2`, versión pinneada) del artefacto descargado y compararlo contra `SHA256SUMS.txt`; si coincide, proceder a instalar; si no coincide, descartar el artefacto, no sobrescribir la instalación existente y mostrar error de corrupción
    - _Design: Components and Interfaces > Updater in-app (Fase 6); Error Handling_
    - _Requirements: 7.4, 7.5, 7.6, 11.1_

  - [x] 13.6 Escribir property test de la compuerta de verificación de checksum
    - **Property 26: La verificación de checksum es una compuerta estricta antes de instalar**
    - **Validates: Requirements 7.4, 7.5, 7.6**

  - [x] 13.7 Implementar el relanzamiento post-instalación y la continuidad de sesión si se declina
    - Ofrecer relanzar tras instalación exitosa; si el usuario declina, mantener la sesión actual operativa con la versión previa y usar la nueva versión en el siguiente inicio
    - _Design: Components and Interfaces > Updater in-app (Fase 6)_
    - _Requirements: 7.7, 7.8_

  - [x] 13.8 Implementar el manejo de fallos de verificación o descarga preservando la versión instalada
    - Ante fallo de verificación o descarga, mostrar mensaje de error descriptivo y mantener la versión actualmente instalada operativa
    - _Design: Error Handling_
    - _Requirements: 7.9_

  - [x] 13.9 Escribir property test de preservación de la versión instalada ante fallo de verificación o descarga
    - **Property 27: Un fallo de verificación o descarga preserva la versión instalada**
    - **Validates: Requirements 7.9**

  - [x] 13.10 Escribir unit tests de camino nominal y borde para `updater.rs`
    - Cubrir un caso de camino nominal (actualización disponible e instalada con éxito) y un caso de error/borde (checksum inválido o versión igual/menor)
    - _Requirements: 10.4_

- [x] 14. Checkpoint - Fase 6
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 15. Fase 7: Empaquetado y distribución
  - [x] 15.1 Configurar `cargo-packager` para `midway-desktop`
    - Fijar la versión exacta del crate `cargo-packager` (con al menos una release publicada en los últimos 12 meses) y configurar `[package.metadata.packager]`/`packager.json` para generar instaladores Windows (NSIS/MSI) y Linux (AppImage/deb) para arquitectura x86_64
    - _Design: Components and Interfaces > Empaquetado y distribución (Fase 7)_
    - _Requirements: 8.1, 8.2, 11.1_

  - [x] 15.2 Adaptar `scripts/release/render-tauri-config.mjs` a la generación de configuración de `cargo-packager`
    - Reemplazar por un script equivalente que preserve identifier, productName, homepage y publisher para los canales estable/beta
    - _Design: Components and Interfaces > Empaquetado y distribución (Fase 7)_
    - _Requirements: 8.5_

  - [x] 15.3 Adaptar `scripts/release/generate-updater-json.mjs` y `scripts/release/generate-checksums.mjs`
    - Ajustar los patrones de nombre de archivo esperados a los que produce `cargo-packager`, preservando la generación de `latest.json`, `latest-beta.json` y `SHA256SUMS.txt`
    - _Design: Components and Interfaces > Empaquetado y distribución (Fase 7)_
    - _Requirements: 8.3, 8.4_

  - [x] 15.4 Adaptar `.github/workflows/ci.yml` y `.github/workflows/release.yml`
    - Reemplazar `tauri-apps/tauri-action` por la invocación de `cargo packager` sobre `midway-desktop`; mantener la matriz Windows/Linux x86_64 y la firma/checksum de artefactos; mantener temporalmente los pasos de Node/npm hasta la Fase 8
    - _Design: Components and Interfaces > Empaquetado y distribución (Fase 7)_
    - _Requirements: 8.5_

  - [x] 15.5 Implementar la detención del pipeline ante fallo de empaquetado en alguna plataforma
    - Si `cargo-packager` falla para alguna plataforma, detener el pipeline, reportar la plataforma y causa del fallo, y evitar la publicación de artefactos parciales o corruptos
    - _Design: Error Handling_
    - _Requirements: 8.6_

  - [x] 15.6 Escribir smoke tests de pipeline de CI para instaladores y artefactos del updater
    - Verificar generación exitosa de instaladores Windows/Linux y presencia de `latest.json`/`latest-beta.json`/`SHA256SUMS.txt` con 1-2 ejemplos representativos
    - _Requirements: 8.2, 8.3, 8.4_

- [x] 16. Checkpoint - Fase 7
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo. Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 10.1, 10.2, 10.3_

- [x] 17. Verificación transversal: resumen por fase y comparación funcional final (Requisito 10)
  - [x] 17.1 Redactar el resumen escrito de equivalencia funcional por fase (Fases 0-8)
    - Para cada funcionalidad migrada, documentar si quedó equivalente, parcial o se decidió resolver distinto respecto a Midway_Tauri, incluyendo la razón de cada decisión distinta, y documentar la ausencia de alternativa de crate cuando aplique (Requisito 11.3)
    - _Requirements: 10.5, 11.3_

  - [x] 17.2 Ejecutar y documentar la comparación funcional manual final (Fase 8)
    - Ejecutar como mínimo los flujos: crear request, guardar request, correr una colección, importar OpenAPI y exportar cURL contra la última versión funcional de Midway_Tauri, marcando cada flujo como equivalente o con diferencias
    - _Requirements: 10.6_

  - [x] 17.3 Redactar el reporte de diferencias encontradas
    - Clasificar cada diferencia encontrada como regresión bloqueante o no bloqueante
    - _Requirements: 10.7_

  - [x] 17.4 Verificar el bloqueo de la limpieza final ante regresiones bloqueantes no resueltas
    - Confirmar que ninguna regresión bloqueante quede sin documentar o sin aceptación explícita antes de proceder a la Fase 8
    - _Requirements: 10.8_

- [x] 18. Fase 8: Limpieza final
  - [x] 18.1 Eliminar el crate `midway` (Tauri) y su configuración asociada
    - Eliminar `src-tauri/` completo: `Cargo.toml`, `Cargo.lock`, `build.rs`, `tauri.conf.json` y variantes (`tauri.release.conf.json`, `tauri.beta.conf.json`, `tauri.windows.conf.json`, `tauri.macos.conf.json`, `tauri.linux.conf.json`), `capabilities/` e `icons/`; solo tras verificar la paridad funcional completa (Tarea 17)
    - _Design: Overview; Architecture > Cargo workspace_
    - _Requirements: 9.1_

  - [x] 18.2 Eliminar el frontend TypeScript y el tooling de npm asociado
    - Eliminar `src/`, `package.json`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `node_modules/` y `tests/ui/`
    - _Requirements: 9.2_

  - [x] 18.3 Actualizar el `Cargo.toml` raíz del workspace y verificar compilación con los crates restantes
    - Quitar `src-tauri` de `members`; verificar que `cargo check`/`cargo build` compilan exitosamente considerando únicamente `midway-core` y `midway-desktop`
    - _Requirements: 9.3, 9.4_

  - [x] 18.4 Finalizar la adaptación de `.github/workflows/` y `.github/release.yml` retirando los pasos de Node/npm
    - Quitar los pasos de build del frontend Vite ahora que `src/` fue eliminado
    - _Design: Components and Interfaces > Empaquetado y distribución (Fase 7)_
    - _Requirements: 8.5_

  - [x] 18.5 Actualizar el `README.md`
    - Eliminar referencias a React, TypeScript, Vite y Tauri como tecnologías activas; actualizar instrucciones de build e instalación para el stack 100% Rust
    - _Requirements: 9.5_

- [x] 19. Checkpoint final - Fase 8
  - Ejecutar `cargo check` y `cargo test` sobre el workspace completo (`midway-core`, `midway-desktop`). Ensure all tests pass, ask the user if questions arise.
  - _Requirements: 9.3, 10.1, 10.2, 10.3_

## Notes

- Las tareas marcadas con `*` son opcionales y pueden omitirse para una entrega más rápida; sin embargo, las tareas de property tests están fuertemente recomendadas dado que verifican las 28 Correctness Properties del diseño.
- Cada tarea referencia requisitos específicos (Requisito.Criterio) y la sección de diseño correspondiente para trazabilidad.
- `midway` (Tauri) y `src/` (TypeScript) deben permanecer funcionales y pasar sus tests existentes durante las Fases 0 a 7 (Requisitos 1.6, 1.7, 11.4, 11.5); solo se eliminan en la Fase 8 (Tarea 18), después del checkpoint de verificación transversal (Tarea 17).
- Todas las dependencias nuevas (iced, iced_aw, iced_highlighter, proptest, semver, sha2, cargo-packager, wiremock/mockito, etc.) deben fijarse con versión exacta, sin rangos abiertos (Requisito 11.1).
- Los checkpoints (Tareas 2, 4, 6, 8, 10, 12, 14, 16, 19) exigen `cargo check` y `cargo test` exitosos sobre el workspace completo antes de avanzar a la fase siguiente (Requisito 10, Criterios 1-3).
- Property tests usan `proptest` con un mínimo de 100 casos por ejecución, y cada test está etiquetado con un comentario `// Feature: tauri-to-iced-migration, Property {number}: {property_text}` según la Testing Strategy del diseño.

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2"] },
    { "id": 2, "tasks": ["1.3", "1.5"] },
    { "id": 3, "tasks": ["1.4", "1.6", "1.7"] },
    { "id": 4, "tasks": ["3.1"] },
    { "id": 5, "tasks": ["3.2", "3.3", "3.11"] },
    { "id": 6, "tasks": ["3.5", "3.9", "3.12", "3.14", "3.16", "3.18"] },
    { "id": 7, "tasks": ["3.4", "3.6", "3.7", "3.8", "3.10", "3.13", "3.15", "3.17", "3.19", "3.20"] },
    { "id": 8, "tasks": ["3.21"] },
    { "id": 9, "tasks": ["5.1"] },
    { "id": 10, "tasks": ["5.2", "5.4", "5.6", "5.8"] },
    { "id": 11, "tasks": ["5.3", "5.5", "5.7", "5.9"] },
    { "id": 12, "tasks": ["7.1"] },
    { "id": 13, "tasks": ["7.2", "7.6", "7.11", "7.12", "7.15"] },
    { "id": 14, "tasks": ["7.3", "7.4", "7.5", "7.7", "7.13", "7.14"] },
    { "id": 15, "tasks": ["7.9"] },
    { "id": 16, "tasks": ["7.8", "7.10"] },
    { "id": 17, "tasks": ["9.1"] },
    { "id": 18, "tasks": ["9.2"] },
    { "id": 19, "tasks": ["9.3", "9.4", "9.5"] },
    { "id": 20, "tasks": ["9.6"] },
    { "id": 21, "tasks": ["9.7"] },
    { "id": 22, "tasks": ["11.1"] },
    { "id": 23, "tasks": ["11.2", "11.4", "11.8", "11.12", "11.14", "11.16"] },
    { "id": 24, "tasks": ["11.3", "11.5", "11.7", "11.9", "11.10", "11.13", "11.15", "11.17"] },
    { "id": 25, "tasks": ["11.11"] },
    { "id": 26, "tasks": ["13.1"] },
    { "id": 27, "tasks": ["13.2", "13.3"] },
    { "id": 28, "tasks": ["13.4", "13.5"] },
    { "id": 29, "tasks": ["13.6", "13.7", "13.8"] },
    { "id": 30, "tasks": ["13.9", "13.10"] },
    { "id": 31, "tasks": ["15.1"] },
    { "id": 32, "tasks": ["15.2", "15.3"] },
    { "id": 33, "tasks": ["15.4"] },
    { "id": 34, "tasks": ["15.5", "15.6"] },
    { "id": 35, "tasks": ["17.1"] },
    { "id": 36, "tasks": ["17.2"] },
    { "id": 37, "tasks": ["17.3"] },
    { "id": 38, "tasks": ["17.4"] },
    { "id": 39, "tasks": ["18.1"] },
    { "id": 40, "tasks": ["18.2"] },
    { "id": 41, "tasks": ["18.3", "18.4", "18.5"] }
  ]
}
```
