# Requirements Document

## Introduction

Midway es un cliente API de escritorio (tipo Postman/Insomnia) actualmente implementado como app Tauri con frontend en React/TypeScript y backend en Rust (`src-tauri/src/`, organizado en `domain/`, `infra/`, `runtime/` y `commands/`). Esta migración reemplaza por completo el frontend Tauri + React/TypeScript por una aplicación de escritorio 100% Rust construida con el framework de UI `iced`, eliminando la capa de comandos IPC de Tauri y llamando a la lógica de dominio directamente en el mismo proceso.

La migración preserva íntegramente la lógica de dominio ya escrita en Rust (capas `domain/`, `infra/`, `runtime/`), la persistencia en SQLite, el almacenamiento de secretos en el keyring del sistema operativo, y toda la funcionalidad visible para el usuario: request composer, tabs de configuración del request, response inspector, workspace panel (Environments, Data, History, Diagnostics, App updates), collection runner, command palette, persistencia de sesión, shortcuts de teclado, updater in-app y empaquetado/distribución multiplataforma.

La migración se ejecuta en 8 fases secuenciales (Fase 0 a Fase 8), cada una verificada por compilación y tests antes de avanzar a la siguiente, sin restricción de tiempo ni de turnos, priorizando completitud y correctitud sobre velocidad. El crate Tauri (`midway`) y el frontend TypeScript (`src/`) deben permanecer funcionales sin romperse durante las Fases 0 a 7, y solo se eliminan en la Fase 8 tras verificar paridad funcional completa.

## Glossary

- **Midway_Desktop**: El nuevo binario/crate Rust que implementa la UI de escritorio usando `iced`, reemplazo funcional de la app Tauri.
- **Midway_Core**: El crate de librería Rust que contiene la lógica de dominio (`domain/`), infraestructura (`infra/`) y runtime de ejecución (`runtime/`), reutilizada sin reescritura desde `src-tauri/src/`.
- **Midway_Tauri**: El crate Tauri existente (`midway`), incluyendo su capa de comandos IPC (`commands/mod.rs`), que se mantiene funcional hasta la Fase 8 y se elimina al final.
- **Workspace_Cargo**: El Cargo workspace raíz que contiene los crates `midway-core`, `midway-desktop` y (temporalmente) `midway`.
- **Request_Composer**: El componente de UI que permite construir y enviar una petición HTTP (método, URL, botón Send, selector de environment, settings del request, tabs de configuración del request).
- **Response_Inspector**: El componente de UI que muestra el resultado de una petición HTTP ejecutada (status, tiempo, tamaño, tabs Body/Headers/Tests).
- **Workspace_Panel**: El panel lateral secundario de la UI que agrupa Environments, Data, History, Diagnostics y App updates.
- **Collection_Runner**: El subsistema que ejecuta secuencialmente un conjunto de requests guardados y reporta progreso y resultados consolidados.
- **Command_Palette**: El componente de UI invocado con Ctrl/Cmd+K que permite ejecutar acciones rápidas y navegar a colecciones/requests.
- **Curl_Importer**: El componente de lógica que parsea un comando cURL pegado y lo convierte en los campos de un request (método, URL, query params, headers, auth, body).
- **Text_Editor_Component**: El componente de edición de texto basado en `text_editor` de iced usado para body de request, response y preview, con resaltado de sintaxis JSON, formateo, lint y búsqueda.
- **Updater**: El subsistema encargado de verificar, descargar, instalar y relanzar la aplicación tras una actualización disponible.
- **Session_Store**: El subsistema de persistencia de estado de sesión (tabs abiertas, tab activa, paneles redimensionados, stack de tabs cerradas, draft activo).
- **Error_Boundary**: El mecanismo de recuperación segura ante errores/paniques de la capa de UI que evita que un fallo en un componente derribe toda la aplicación.
- **Fase**: Una etapa secuencial y delimitada del plan de migración (Fase 0 a Fase 8), cada una con criterios de verificación propios.

## Requirements

### Requisito 1: Reestructuración como Cargo workspace (Fase 0)

**User Story:** Como desarrollador del proyecto, quiero reestructurar el repositorio como un Cargo workspace con un crate de librería separado para la lógica de dominio, para poder construir un nuevo frontend en iced sin duplicar ni reescribir lógica ya probada.

#### Acceptance Criteria

1. THE Workspace_Cargo SHALL declarar los crates `midway-core`, `midway-desktop` y `midway` como miembros de un único Cargo workspace.
2. WHEN se crea el crate `midway-core`, THE Workspace_Cargo SHALL contener el código de `domain/`, `infra/` y `runtime/` movido desde `src-tauri/src/` sin reescritura de lógica, ajustando únicamente rutas de módulos y visibilidad `pub` necesaria para su consumo externo.
3. THE Midway_Core SHALL exponer como públicas todas las funciones, tipos y estructuras que `midway-desktop` y `midway` necesiten invocar directamente sin pasar por una capa de comandos IPC, de forma que dichos crates compilen exitosamente al referenciar dichos elementos.
4. WHEN se crea el crate `midway-desktop`, THE Workspace_Cargo SHALL configurar dicho crate como binario dependiente de `midway-core` y de `iced`.
5. THE Midway_Desktop SHALL fijar en su `Cargo.toml` una versión exacta de `iced`, sin operadores de rango (`^`, `~`, `*`) ni comodines, de forma que dicha versión pinneada sea verificable de forma idéntica en `Cargo.toml` y en `Cargo.lock` en cualquier momento posterior a la migración.
6. WHILE las Fases 0 a 7 estén en curso, THE Midway_Tauri SHALL permanecer como miembro del Workspace_Cargo y SHALL compilar exitosamente mediante `cargo check` sin errores.
7. WHILE las Fases 0 a 7 estén en curso, THE Midway_Tauri SHALL pasar sin fallos la suite de tests automatizados existente correspondiente al crate `midway` en cada Fase.
8. WHEN se ejecuta `cargo check` sobre el Workspace_Cargo completo, THE Workspace_Cargo SHALL compilar sin errores para los tres crates miembros.
9. IF el proceso de extracción de `domain/`, `infra/` o `runtime/` requiere modificar una firma pública existente, THEN THE Midway_Core SHALL mantener el comportamiento funcional original de dicha firma, verificado por la ausencia de regresiones en la suite de tests automatizados existente que cubre dicha firma.
10. THE Midway_Core SHALL compilar y funcionar sin declarar una dependencia directa ni transitiva sobre `tauri` o sus crates asociados, reubicando en `midway-desktop` o en `midway` cualquier uso existente de dichos crates dentro de `domain/`, `infra/` o `runtime/`, incluyendo el uso actual de `tauri::async_runtime::spawn` en `runtime/secret_executor.rs`.

### Requisito 2: Request composer y response inspector (Fase 1)

**User Story:** Como usuario de Midway, quiero poder construir y enviar una petición HTTP y ver su respuesta desde la app en iced, para reproducir el flujo principal (Método + URL + Send) que ya usaba en la versión Tauri.

#### Acceptance Criteria

1. THE Request_Composer SHALL presentar un selector de método HTTP con las opciones `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD` y `OPTIONS`.
2. THE Request_Composer SHALL presentar una barra de URL editable como elemento dominante del flujo principal.
3. THE Request_Composer SHALL presentar un botón Send para ejecutar la petición HTTP configurada en la tab activa.
4. THE Request_Composer SHALL presentar un selector de environment compacto para seleccionar el environment activo.
5. THE Request_Composer SHALL presentar un control de settings del request accesible mediante un ícono de engranaje.
6. WHEN el usuario pega en la barra de URL un texto que comienza con el comando `curl` (u opcionalmente `curl.exe`) y del cual puede extraerse una URL, THE Curl_Importer SHALL considerarlo un comando cURL válido y SHALL extraer método, URL, query params, headers, autenticación básica o bearer, y body, completando los campos correspondientes del Request_Composer.
7. IF la tab activa del Request_Composer está vacía (método `GET`, URL vacía, y sin query params, headers, body ni autenticación configurados) al pegar un comando cURL válido, THEN THE Curl_Importer SHALL completar los campos de la tab activa con los datos extraídos.
8. IF la tab activa del Request_Composer contiene algún dato no vacío (método distinto de `GET`, o algún valor en URL, query params, headers, body o autenticación) al pegar un comando cURL válido, THEN THE Curl_Importer SHALL crear una nueva tab con los datos extraídos sin sobrescribir la tab activa existente.
9. IF el texto pegado en la barra de URL no comienza con el comando `curl` (u opcionalmente `curl.exe`) o no contiene una URL de la cual pueda extraerse un valor, THEN THE Curl_Importer SHALL insertar el texto pegado en la barra de URL como texto plano sin invocar ninguna extracción de campos.
10. WHEN una petición HTTP finaliza, THE Response_Inspector SHALL mostrar el código de status, el tiempo de respuesta en milisegundos y el tamaño del payload recibido en bytes.
11. THE Response_Inspector SHALL presentar tabs separadas para Body, Headers y Tests del resultado de la petición ejecutada, donde la tab Tests SHALL mostrar, para cada assertion configurada en la tab Tests del request (Requisito 3), si su evaluación resultó aprobada o fallida.
12. THE Text_Editor_Component SHALL usar el widget `text_editor` de iced para la edición y visualización de body de request, body de response y preview, con resaltado de sintaxis JSON.
13. WHEN el usuario activa la acción de formateo sobre contenido JSON válido dentro del Text_Editor_Component, THE Text_Editor_Component SHALL reformatear dicho contenido aplicando indentación estándar de JSON.
14. IF el contenido JSON dentro del Text_Editor_Component es inválido, THEN THE Text_Editor_Component SHALL señalar el error de lint indicando la línea y columna donde ocurre y una descripción del motivo del error.
15. THE Text_Editor_Component SHALL ofrecer una función de búsqueda de texto dentro del contenido mostrado, resaltando todas las coincidencias encontradas y permitiendo navegar secuencialmente entre ellas.
16. FOR ALL comandos cURL válidos soportados por `src/lib/curl.ts` de referencia, parsear el comando con Curl_Importer y reconstruir sus campos equivalentes SHALL producir una configuración de request equivalente a la generada por la implementación TypeScript de referencia.
17. WHEN el usuario selecciona el botón Send, THE Request_Composer SHALL ejecutar la petición HTTP configurada en la tab activa mediante llamadas directas a Midway_Core.
18. WHEN el usuario activa el control de settings del request, THE Request_Composer SHALL mostrar una vista previa (preview) de la petición HTTP resultante, incluyendo método, URL final con query params, headers y body vigentes en la tab activa.
19. IF el texto pegado en la barra de URL comienza con el comando `curl` (u opcionalmente `curl.exe`) pero no puede parsearse completamente como un comando cURL (por ejemplo, por un flag que requiere un valor y no lo tiene), THEN THE Curl_Importer SHALL mostrar un mensaje de error indicando el motivo sin modificar el contenido existente de la barra de URL.

### Requisito 3: Tabs de configuración del request (Fase 2)

**User Story:** Como usuario de Midway, quiero configurar parámetros, headers, autenticación, body y tests de un request en tabs separadas, para organizar la configuración avanzada sin saturar el flujo principal.

#### Acceptance Criteria

1. THE Request_Composer SHALL presentar tabs de configuración del request para Params, Headers, Auth, Body y Tests.
2. WHEN el método HTTP seleccionado es `GET`, `HEAD` u `OPTIONS`, THE Request_Composer SHALL mostrar la tab Params como tab seleccionada por defecto, salvo que el usuario haya seleccionado manualmente otra tab para el request actual.
3. WHEN el método HTTP seleccionado es `POST`, `PUT`, `PATCH` o `DELETE`, THE Request_Composer SHALL mostrar la tab Body como tab seleccionada por defecto, salvo que el usuario haya seleccionado manualmente otra tab para el request actual.
4. THE Request_Composer SHALL permitir seleccionar, dentro de la tab Auth, uno de los siguientes tipos de autenticación: None, Bearer, Basic o ApiKey.
5. WHEN el usuario selecciona el tipo de autenticación Bearer, THE Request_Composer SHALL presentar un campo para el token.
6. WHEN el usuario selecciona el tipo de autenticación Basic, THE Request_Composer SHALL presentar campos para usuario y contraseña.
7. WHEN el usuario selecciona el tipo de autenticación ApiKey, THE Request_Composer SHALL presentar campos para nombre de la clave, valor y ubicación (header o query param).
8. THE Request_Composer SHALL permitir agregar, editar, eliminar y alternar el estado habilitado (`enabled: bool`) de cada fila de pares clave/valor en la tab Params y en la tab Headers.
9. THE Request_Composer SHALL permitir definir assertions de test sobre la respuesta en la tab Tests, donde cada assertion está compuesta por un source (Status, Header, BodyText, JsonPointer o FinalUrl), un operator (Equals, Contains, NotContains, Exists, NotExists, GreaterOrEqual o LessOrEqual), un selector opcional y un expected, reutilizando el motor de assertions de `domain/testing.rs` sin reescritura de su lógica.
10. WHEN el usuario cambia el tipo de autenticación seleccionado en la tab Auth, THE Request_Composer SHALL descartar los valores previamente ingresados en los campos correspondientes al tipo de autenticación anterior.

### Requisito 4: Workspace panel lateral (Fase 3)

**User Story:** Como usuario de Midway, quiero acceder a Environments, Data, History, Diagnostics y App updates desde un panel lateral secundario, para gestionar aspectos no esenciales al flujo principal sin saturarlo.

#### Acceptance Criteria

1. THE Workspace_Panel SHALL presentar secciones separadas para Environments, Data, History, Diagnostics y App updates.
2. THE Workspace_Panel SHALL permitir, dentro de la sección Environments, crear, editar, eliminar y seleccionar como activo un environment, de forma que como máximo un environment esté activo a la vez, permitiendo un máximo de 100 environments por workspace y un nombre de environment de hasta 100 caracteres.
3. WHEN el usuario solicita exportar datos en la sección Data, THE Workspace_Panel SHALL ofrecer exportación en formato nativo workspace v1 y en formato Postman Collection v2.1.
4. WHEN el usuario solicita importar datos en la sección Data desde un archivo o payload pegado, THE Workspace_Panel SHALL soportar los formatos nativo workspace v1, Postman Collection v2.1 y OpenAPI v3 en JSON o YAML, para archivos o payloads de hasta 10 MB de tamaño.
5. IF el contenido importado en la sección Data no corresponde a ninguno de los formatos soportados, está corrupto, o excede el tamaño máximo permitido, THEN THE Workspace_Panel SHALL mostrar un mensaje de error descriptivo sin modificar el estado de datos existente.
6. THE Workspace_Panel SHALL listar en la sección History las peticiones HTTP ejecutadas previamente, ordenadas cronológicamente de la más reciente a la más antigua, reteniendo un máximo de 500 entradas.
7. THE Workspace_Panel SHALL mostrar en la sección Diagnostics los errores y crashes capturados localmente por la aplicación, ordenados del más reciente al más antiguo, reteniendo un máximo de 200 entradas.
8. THE Workspace_Panel SHALL mostrar en la sección App updates el estado actual del Updater (sin actualizaciones, actualización disponible, descargando, lista para instalar).
9. IF el usuario intenta crear o renombrar un environment con un nombre que ya existe en el workspace, THEN THE Workspace_Panel SHALL rechazar la operación y SHALL mostrar un mensaje de error indicando que el nombre está duplicado, sin modificar los environments existentes.
10. IF el usuario elimina el environment actualmente seleccionado como activo, THEN THE Workspace_Panel SHALL dejar el selector de environment sin ningún environment activo y SHALL indicar visualmente que no hay environment seleccionado.
11. WHEN una importación válida en la sección Data introduce un environment, request o colección con el mismo nombre que un elemento ya existente en el workspace, THE Workspace_Panel SHALL conservar ambos elementos asignando al elemento importado un nombre diferenciado, sin sobrescribir ni eliminar el elemento existente.

### Requisito 5: Collection runner (Fase 4)

**User Story:** Como usuario de Midway, quiero ejecutar secuencialmente todos los requests guardados en una colección y ver un reporte consolidado, para validar múltiples endpoints sin ejecutarlos manualmente uno por uno.

#### Acceptance Criteria

1. WHEN el usuario inicia la ejecución de una colección, THE Collection_Runner SHALL ejecutar los requests guardados de dicha colección de forma secuencial, uno a la vez, en el orden en que dichos requests están guardados dentro de la colección, reutilizando la lógica existente en `domain/runner.rs` sin reescritura.
2. WHILE la ejecución de una colección está en curso, THE Collection_Runner SHALL actualizar el progreso reportado cada vez que un request inicia su ejecución y cada vez que un request finaliza su ejecución, indicando el identificador o nombre del request en ejecución, la cantidad de requests completados y el total de requests de la colección.
3. WHEN la ejecución de una colección finaliza sin haber sido cancelada, THE Collection_Runner SHALL presentar un reporte consolidado que incluya, para cada request ejecutado, un indicador de éxito o fallo, el código de status HTTP recibido (si se recibió respuesta), el tiempo de respuesta, el resultado de cada assertion evaluada, y, para el conjunto de la colección, el total de requests ejecutados, la cantidad exitosa y la cantidad fallida.
4. WHERE el usuario define un environment de override antes de correr la colección, THE Collection_Runner SHALL usar dicho environment de override durante toda la ejecución en lugar del environment activo por defecto.
5. IF un request dentro de la colección falla durante su ejecución porque la llamada HTTP no pudo completarse (error de red, timeout o falla de conexión) o porque al menos una assertion definida para dicho request resulta fallida, THEN THE Collection_Runner SHALL registrar el fallo junto con su motivo en el reporte consolidado y SHALL continuar con el siguiente request de la secuencia.
6. WHEN el usuario inicia la ejecución de una colección sin haber definido un environment de override, THE Collection_Runner SHALL usar el environment activo por defecto durante toda la ejecución.
7. IF la colección a ejecutar no tiene ningún request guardado al momento de iniciar la ejecución, THEN THE Collection_Runner SHALL presentar de inmediato un reporte consolidado indicando cero requests ejecutados, sin realizar ninguna llamada HTTP.
8. WHEN el usuario cancela una ejecución de colección en curso, THE Collection_Runner SHALL detener la ejecución de los requests restantes de la secuencia y SHALL presentar un reporte consolidado que refleje únicamente los requests ejecutados hasta el momento de la cancelación.

### Requisito 6: Command palette, persistencia de sesión y shortcuts (Fase 5)

**User Story:** Como usuario de Midway, quiero usar una command palette, conservar mi sesión de trabajo entre reinicios y usar los mismos shortcuts de teclado que en la versión Tauri, para mantener mi flujo de trabajo sin fricciones tras la migración.

#### Acceptance Criteria

1. WHEN el usuario presiona Ctrl/Cmd+K, THE Command_Palette SHALL abrirse mostrando acciones rápidas, colecciones y requests disponibles para navegación o ejecución.
2. WHEN el usuario selecciona una acción, colección o request desde el Command_Palette, THE Midway_Desktop SHALL ejecutar la acción correspondiente y SHALL cerrar el Command_Palette.
3. THE Session_Store SHALL guardar automáticamente (autosave) el contenido del draft activo dentro de un máximo de 2 segundos desde la última modificación realizada por el usuario.
4. WHEN la aplicación se reabre tras un cierre normal, THE Session_Store SHALL restaurar la sesión previa, incluyendo tabs abiertas, tab activa y paneles redimensionados, a partir del último estado guardado por el autosave descrito en el Criterio 3.
5. THE Session_Store SHALL mantener un stack de tabs cerradas de hasta 20 tabs, de forma que al cerrar una tab adicional estando el stack lleno se descarte la entrada más antigua, y que reabrir una tab cerrada la extraiga del stack en orden LIFO (la más recientemente cerrada se reabre primero).
6. IF el usuario intenta cerrar una tab con cambios sin guardar, THEN THE Midway_Desktop SHALL mostrar un aviso de unsaved changes con las opciones Guardar, Descartar y Cancelar, de forma que Guardar persista los cambios y cierre la tab, Descartar cierre la tab sin persistir los cambios, y Cancelar mantenga la tab abierta sin cerrarla.
7. IF la aplicación se cierra de forma inesperada (crash), THEN THE Session_Store SHALL permitir recuperar en el siguiente inicio el estado de sesión guardado más reciente por el mecanismo de autosave descrito en el Criterio 3.
8. THE Midway_Desktop SHALL soportar los siguientes shortcuts de teclado con su acción asociada: Ctrl+Enter (Send), Ctrl+S (Guardar), Ctrl+Shift+N (Nuevo request), Ctrl+Shift+P (Preview), Ctrl+. (Tools/Workspace), Ctrl+K (Command Palette), Ctrl+W (Cerrar tab activa), Ctrl+Shift+T (Reabrir tab), Alt+1 a Alt+9 (Ir a la tab abierta correspondiente) y Esc (Cerrar panel o settings abiertos).
9. IF un componente de la UI produce un error o panic durante su renderizado o manejo de eventos, THEN THE Error_Boundary SHALL reemplazar dicho componente por un mensaje de error visible, SHALL registrar el evento en diagnostics, y el resto de la aplicación SHALL continuar respondiendo a las interacciones del usuario sin cerrarse.
10. IF el estado de sesión a restaurar está corrupto o es incompatible con la versión actual de Midway_Desktop, THEN THE Session_Store SHALL descartar dicho estado, SHALL iniciar una sesión vacía, y SHALL notificar al usuario que la sesión previa no pudo restaurarse.

### Requisito 7: Updater in-app (Fase 6)

**User Story:** Como usuario de Midway, quiero que la aplicación pueda verificar, descargar e instalar actualizaciones desde dentro de la propia app, para mantenerme en la última versión sin pasos manuales adicionales.

#### Acceptance Criteria

1. THE Updater SHALL implementarse usando una solución nativa en Rust que reemplace a `tauri-plugin-updater`, sin depender de infraestructura de Tauri.
2. WHEN el usuario solicita verificar actualizaciones, THE Updater SHALL consultar los artefactos `latest.json` o `latest-beta.json` (según el canal configurado) publicados en la fuente de actualizaciones definida en el Requisito 8, SHALL comparar la versión ahí indicada con la versión actualmente instalada usando versionado semántico, y SHALL informar al usuario si existe una versión más reciente disponible.
3. WHEN el usuario solicita descargar una actualización disponible, THE Updater SHALL descargar el artefacto correspondiente a la plataforma actual y SHALL reportar el progreso de descarga como un valor porcentual entre 0% y 100%, actualizado con una frecuencia mínima de 1 actualización por segundo.
4. WHEN la descarga de una actualización finaliza, THE Updater SHALL verificar la integridad del artefacto descargado calculando su checksum SHA256 y comparándolo con el valor correspondiente publicado en `SHA256SUMS.txt` (Requisito 8) antes de proceder con la instalación.
5. IF la verificación de checksum SHA256 del artefacto descargado no coincide con el valor esperado, THEN THE Updater SHALL descartar el artefacto descargado, SHALL no sobrescribir la instalación existente, y SHALL mostrar un mensaje de error indicando que la actualización descargada está corrupta.
6. WHEN la verificación de checksum SHA256 de la actualización descargada finaliza correctamente, THE Updater SHALL instalar la actualización descargada.
7. WHEN la instalación de una actualización finaliza correctamente, THE Updater SHALL ofrecer al usuario la opción de relanzar la aplicación con la nueva versión.
8. WHEN el usuario declina relanzar la aplicación inmediatamente después de una instalación exitosa, THE Updater SHALL mantener la sesión actual operativa con la versión previa en ejecución, y THE Midway_Desktop SHALL utilizar la versión instalada la próxima vez que la aplicación se inicie.
9. IF la verificación o la descarga de una actualización falla, THEN THE Updater SHALL mostrar un mensaje de error descriptivo y SHALL mantener la versión actualmente instalada operativa.

### Requisito 8: Empaquetado y distribución (Fase 7)

**User Story:** Como responsable de release del proyecto, quiero generar instaladores para Windows y Linux del nuevo binario en iced preservando el pipeline de release existente, para no perder la infraestructura de distribución ya construida.

#### Acceptance Criteria

1. THE Midway_Desktop SHALL contar con una herramienta de empaquetado (por ejemplo `cargo-packager` u otra alternativa equivalente evaluada) que reemplace a `tauri-build`/Tauri CLI para generar instaladores, y dicha herramienta SHALL tener al menos una release publicada en los últimos 12 meses.
2. THE Midway_Desktop SHALL generar, para arquitectura x86_64, un instalador para Windows y un instalador para Linux, cada uno de los cuales SHALL instalarse sin errores y SHALL permitir lanzar la aplicación correctamente en un sistema limpio de la plataforma correspondiente.
3. THE Midway_Desktop SHALL preservar, dentro del pipeline de release, la generación de los artefactos `latest.json` y `latest-beta.json` para el updater, de forma que el Updater (Requisito 7) pueda consumir dichos artefactos sin modificaciones.
4. THE Midway_Desktop SHALL preservar, dentro del pipeline de release, la generación del archivo de checksums `SHA256SUMS.txt` para los nuevos artefactos de instalación.
5. THE Midway_Desktop SHALL adaptar los workflows de CI y release existentes en `.github/workflows/` y `.github/release.yml`, y los scripts en `scripts/release/*`, para construir y empaquetar `midway-desktop` en lugar de `midway` (Tauri), y dichos workflows y scripts adaptados SHALL ejecutarse de forma exitosa de extremo a extremo para `midway-desktop`.
6. IF la herramienta de empaquetado falla durante la generación de un instalador para alguna plataforma, THEN THE Midway_Desktop SHALL detener el pipeline de release, SHALL reportar un error indicando la plataforma afectada y la causa del fallo, y SHALL evitar la publicación de artefactos parciales o corruptos.

### Requisito 9: Limpieza final (Fase 8)

**User Story:** Como desarrollador del proyecto, quiero eliminar el código y tooling de Tauri/React una vez verificada la paridad funcional completa, para dejar el repositorio como una app 100% Rust sin código muerto.

#### Acceptance Criteria

1. IF la paridad funcional completa entre Midway_Tauri y Midway_Desktop fue verificada y reportada según el Requisito 10, THEN THE Workspace_Cargo SHALL eliminar el crate `midway` (Tauri) incluyendo el directorio `src-tauri/` completo y su configuración de crate asociada: `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json` y sus variantes (`tauri.release.conf.json`, `tauri.beta.conf.json`, `tauri.windows.conf.json`, `tauri.macos.conf.json`, `tauri.linux.conf.json`), `src-tauri/capabilities/` y `src-tauri/icons/`.
2. WHEN se elimina el crate `midway`, THE Workspace_Cargo SHALL eliminar el directorio `src/` (TypeScript/React) y el tooling de npm asociado, incluyendo `package.json`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `node_modules/` y `tests/ui/`.
3. WHEN se completa la eliminación de los archivos y directorios especificados en los Criterios 1 y 2, THE Workspace_Cargo SHALL compilar exitosamente mediante `cargo check` o `cargo build` considerando únicamente los crates restantes `midway-core` y `midway-desktop`.
4. WHEN se completa la limpieza final, THE Midway_Desktop SHALL ser el único punto de entrada de la aplicación de escritorio dentro del Workspace_Cargo.
5. WHEN se completa la limpieza final, THE Workspace_Cargo SHALL actualizar el README.md eliminando las referencias a React, TypeScript, Vite y Tauri como tecnologías activas del proyecto, y actualizando las instrucciones de build e instalación para reflejar el nuevo stack 100% Rust.

### Requisito 10: Verificación por fase y comparación funcional final

**User Story:** Como responsable técnico de la migración, quiero que cada fase se verifique por compilación y tests antes de avanzar, y que al final se compare funcionalmente contra la app Tauri original, para garantizar que no se pierde funcionalidad ni se introducen regresiones.

#### Acceptance Criteria

1. WHEN se completa el trabajo de cualquier Fase de la migración (0 a 7), THE Workspace_Cargo SHALL compilar exitosamente mediante `cargo check` o `cargo build` antes de iniciar la Fase siguiente.
2. WHEN se completa el trabajo de cualquier Fase de la migración (0 a 7), THE Workspace_Cargo SHALL ejecutar `cargo test` y no deberá reportar ningún test fallido antes de iniciar la Fase siguiente.
3. IF `cargo check`, `cargo build` o `cargo test` falla al completar el trabajo de cualquier Fase de la migración (0 a 7), THEN THE Workspace_Cargo SHALL bloquear el avance a la Fase siguiente hasta que dicha falla sea corregida y las tres verificaciones se ejecuten exitosamente.
4. WHEN se reescribe en Rust lógica UI-adyacente previamente implementada en TypeScript (por ejemplo parseo de cURL, formateo JSON), THE Midway_Desktop SHALL contar con tests unitarios de Rust que cubran dicha lógica reescrita, incluyendo como mínimo un caso de camino nominal y al menos un caso de error o borde, y dichos tests SHALL pasar exitosamente.
5. WHEN se completa cualquier Fase de la migración (0 a 8), THE Midway_Desktop SHALL contar con un resumen escrito que indique, para cada funcionalidad migrada, si quedó equivalente (el comportamiento observable coincide con el de Midway_Tauri para las mismas entradas), parcial (el comportamiento observable difiere de Midway_Tauri en al menos un escenario documentado) o se decidió resolver distinto respecto a la versión Tauri, incluyendo la razón de cada decisión distinta.
6. WHEN se completa la Fase 8, THE Midway_Desktop SHALL haber sido comparado manualmente contra la última versión funcional de Midway_Tauri ejecutando como mínimo los flujos: crear request, guardar request, correr una colección, importar OpenAPI y exportar cURL, marcando explícitamente cada uno de estos flujos como equivalente o con diferencias respecto a Midway_Tauri.
7. WHEN se completa la comparación funcional manual final, THE Midway_Desktop SHALL contar con un reporte de diferencias encontradas entre su comportamiento y el de Midway_Tauri, si las hubiera, clasificando cada diferencia como regresión bloqueante o no bloqueante.
8. IF el reporte de diferencias identifica una regresión bloqueante que no está documentada o no ha sido aceptada explícitamente por el responsable técnico de la migración, THEN THE Workspace_Cargo SHALL bloquear la limpieza final descrita en el Requisito 9 hasta que dicha regresión sea resuelta o aceptada explícitamente.

### Requisito 11: Restricciones técnicas transversales

**User Story:** Como responsable técnico de la migración, quiero que se respeten restricciones de versionado de dependencias, elección de crates y compatibilidad durante todas las fases, para mantener la estabilidad y mantenibilidad del proyecto migrado.

#### Acceptance Criteria

1. THE Workspace_Cargo SHALL declarar todas las dependencias nuevas introducidas por la migración con versiones exactas o pinneadas, sin rangos abiertos, en cada `Cargo.toml`.
2. IF existe un crate publicado en crates.io que cubra una necesidad de widget o funcionalidad del ecosistema de iced (por ejemplo `iced_highlighter` para resaltado de sintaxis, `iced_aw` para date pickers o tabs adicionales), que no esté archivado ni marcado como deprecado, y que haya tenido al menos un release publicado en los últimos 12 meses, THEN THE Midway_Desktop SHALL usar dicho crate en lugar de implementar un widget equivalente desde cero.
3. IF no existe ningún crate que cumpla las condiciones del Criterio 2 (publicado en crates.io, no archivado ni deprecado, con al menos un release en los últimos 12 meses) para una necesidad de widget o funcionalidad específica, THEN THE Midway_Desktop SHALL implementar dicha funcionalidad de forma manual, documentando la ausencia de alternativa en el resumen escrito de la Fase correspondiente según el Requisito 10 (Criterio 5).
4. WHILE las Fases 0 a 7 estén en curso, THE Midway_Tauri SHALL compilar sin errores mediante `cargo check` o `cargo build`, sin verse afectado por los cambios introducidos para `midway-desktop`.
5. WHILE las Fases 0 a 7 estén en curso, THE Midway_Tauri SHALL pasar su suite de tests existente sin fallos nuevos introducidos por los cambios realizados para `midway-desktop`.
6. THE Workspace_Cargo SHALL mantener SQLite, mediante la infraestructura ya existente en `infra/sqlite_repository.rs`, como motor de persistencia, sin introducir un motor de persistencia alternativo.
