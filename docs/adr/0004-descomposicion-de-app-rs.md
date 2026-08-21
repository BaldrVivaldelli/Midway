# ADR 0004 — Plan de descomposición de `app.rs` por grupos de mensajes

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 7.4 (con efecto sobre 5.3, 8.10, 8.11, 14.3)
- **Evidencia**: `midway-desktop/src/app.rs` en el HEAD del ADR 0001
  (`1fc270326e5c304f24ce08fe1f528da33a007d3e`), medido con
  `docs/baseline-evidence/attribute_app_rs.py`, `fn_inventory.py`, `count_variants.py` y
  `count_test_lines.py`; citas `archivo:línea` verificadas con
  `docs/baseline-evidence/verify_adr_citations.py`

## Contexto

`midway-desktop/src/app.rs` tiene 10 750 líneas. Los bloques `#[cfg(test)]` son **ocho
módulos, todos de nivel superior**, con estos rangos exactos (anotación `#[cfg(test)]` a
llave de cierre, ambas inclusive):

| Módulo de test | Rango | Líneas |
| --- | --- | --- |
| `panel_resize_tests` | 2675-2809 | 135 |
| `import_name_collision_tests` | 5179-5230 | 52 |
| `debug_layout_tests` | 5468-5487 | 20 |
| `tests` | 5819-9976 | 4 158 |
| `error_boundary_tests` | 9978-10246 | 269 |
| `resolve_startup_collection_tests` | 10249-10349 | 101 |
| `navigation_toggle_preserves_composer_state_tests` | 10352-10575 | 224 |
| `collection_selection_workspace_property_tests` | 10578-10750 | 173 |

Suma exacta: **5 132** líneas de test y **5 618** de producción. `mod tests` (5819-9976)
**no anida** a los cuatro módulos que lo siguen: los cinco son hermanos de nivel superior.
Una versión previa de este ADR afirmaba lo contrario; queda corregido acá.

Nota de método, porque los números conviven en dos documentos.
`docs/baseline-evidence/count_test_lines.py` reporta `test=5140 prod=5610` y es el valor
citado en `docs/product-audit.md` §5. Su contador de llaves es ingenuo (no distingue llaves
dentro de literales ni de comentarios), así que fusiona los últimos cinco bloques en un
único span `5819-10751` y con eso absorbe 7 líneas separadoras entre bloques (9977, 10247,
10248, 10350, 10351, 10576, 10577) más una línea fantasma al final del archivo: 5 132 + 7 +
1 = 5 140. La diferencia de 8 líneas es de método de conteo, no de contenido, y ninguna
conclusión de este ADR depende de ellas. Los valores exactos son 5 132 / 5 618; los valores
del script son 5 140 / 5 610. Se registran los dos en vez de armonizarlos en silencio. La
§5 de `docs/product-audit.md` atribuye a este ADR "~5 132 de test y ~5 619 de producción":
ese segundo número es un desliz aritmético (10 750 − 5 132 = 5 618) que la próxima pasada
sobre ese documento debería corregir.

El archivo contiene simultáneamente el `Message` raíz, las catorce enumeraciones de
mensajes por área, el struct `Midway`, los handlers de `update`, sus auxiliares `handle_*`,
las funciones `async` de efecto, `view`, `subscription` y la aritmética de paneles.

El `Message` raíz **ya está agrupado por área**. Verificado leyendo `app.rs:58-88`: quince
variantes, en este orden exacto.

| # | Variante del `Message` raíz | Payload | Notas del fuente |
| --- | --- | --- | --- |
| 1 | `RequestComposer` | `RequestComposerMessage` | — |
| 2 | `ResponseInspector` | `ResponseInspectorMessage` | — |
| 3 | `Workspace` | `WorkspaceMessage` | — |
| 4 | `Runner` | `RunnerMessage` | — |
| 5 | `Palette` | `PaletteMessage` | — |
| 6 | `Theme` | `ThemeMessage` | — |
| 7 | `Session` | `SessionMessage` | — |
| 8 | `Updater` | `UpdaterMessage` | `#[allow(dead_code)]`, scaffolding |
| 9 | `Keyboard` | `KeyboardMessage` | — |
| 10 | `ActivityBar` | `ActivityBarMessage` | — |
| 11 | `Tree` | `TreeMessage` | `#[allow(dead_code)]`, scaffolding |
| 12 | `WorkspaceCrud` | `WorkspaceCrudMessage` | — |
| 13 | `TopBar` | `TopBarMessage` | — |
| 14 | `PanelResize` | `PanelResizeMessage` | — |
| 15 | `Tick` | *(sin payload)* | `#[allow(dead_code)]`, scaffolding |

Los quince grupos se confirman en el fuente, con tres precisiones que el diseño no
registraba:

1. `Tick` **no tiene enum propio**: es una variante sin payload que hoy se enruta a
   `Task::none()` (`app.rs:1802`). No es un grupo extraíble; es un punto de entrada de
   subscription todavía sin cablear.
2. `Updater` tampoco tiene handler: `Message::Updater(_) => Task::none()`
   (`app.rs:1815`). Su lógica real ya vive en `midway-desktop/src/updater.rs` (2 311
   líneas), sin conexión al `update` raíz.
3. `Tree` es el único grupo cuyo handler **ya está fuera de `app.rs`**:
   `crate::ui::request_tree_pane::update_tree` (`ui/request_tree_pane.rs:226-408`, 183
   líneas). Pero no es una vertical: recibe `&mut Midway` completo y devuelve
   `Task<Message>`, así que el `TreeMessage` sigue definido en `app.rs:869-900` y el
   acoplamiento al estado global permanece. Es un archivo movido, no una frontera.

El mismo patrón se repite en `src/ui/`: `top_bar.rs`, `activity_bar.rs`,
`response_inspector.rs`, `request_tree_pane.rs`, `request_composer.rs`,
`workspace_panel.rs`, `command_palette.rs`, `save_request_modal.rs`,
`unsaved_changes_modal.rs` y `onboarding.rs` reciben `&Midway` (o `&mut Midway`) y hablan el
`Message` raíz. Son **cortes de vista**, no verticales: leen el estado global. Esa es
precisamente la forma que este spec no quiere repetir.

### Atribución de líneas de producción por grupo

Medida con `docs/baseline-evidence/attribute_app_rs.py`, que asigna cada función de
producción de nivel superior a un grupo o a la raíz de composición, y **falla si alguna
función queda sin clasificar**. La columna de líneas cuenta el handler más sus auxiliares
`handle_*`, sus funciones `async` de efecto y sus funciones puras atribuibles.

| Grupo | Variantes del enum | Líneas del enum | Líneas de producción atribuidas |
| --- | --- | --- | --- |
| `RequestComposer` | 51 | 128-394 | 1 174 |
| `Workspace` | 19 | 404-493 | 672 |
| `WorkspaceCrud` | 10 | 829-842 | 349 |
| `ActivityBar` | 8 | 846-865 | 128 |
| `PanelResize` | 6 | 103-116 | 122 |
| `Palette` | 4 | 540-562 | 116 |
| `Keyboard` | 5 | 620-669 | 81 |
| `Runner` | 4 | 502-532 | 75 |
| `Session` | 2 | 572-588 | 49 |
| `TopBar` | 3 | 904-913 | 41 |
| `ResponseInspector` | 1 | 397-401 | 12 |
| `Theme` | 1 | 565-569 | 9 |
| `Updater` | 1 | 591-596 | 1 (routing a `Task::none()`) |
| `Tick` | — | — | 1 (routing a `Task::none()`) |
| `Tree` | 13 | 869-900 | 0 en `app.rs`: 183 ya en `request_tree_pane.rs` |

Suma atribuida a grupos: **2 830** líneas de producción. El resto — 2 780 contra el
`prod=5610` del script, 2 788 contra el conteo exacto de 5 618 — es la raíz de composición
y sus declaraciones: 560 líneas en funciones (`update`, `view`, `subscription`,
`guarded_update`, `guarded_view`, `restore_pending_session`, `main_content_pane`,
`debug_content`, `debug_pane_layout`, `debug_split_content`, `test_content`,
`resolve_startup_collection`, `panic_payload_message`) más el `Message` raíz, los catorce
enums de mensajes, los structs de estado, los `impl` y los `use`. Ese resto es el que
legítimamente se queda.

Nota de consistencia: la sección 6 de `docs/product-audit.md` cita para estos mismos grupos
los valores aproximados que anticipaba el diseño (~1 316 para `RequestComposer`, ~824 para
`Workspace`, ~12 para `update_theme`). Los valores de este ADR son los **medidos** con el
script, y son los que rigen: 1 174, 672 y 9. La divergencia queda registrada acá para que
la próxima pasada sobre `product-audit.md` la corrija en vez de arrastrarla.

Sin un plan escrito, la descomposición se resuelve por intuición en cada revisión, con dos
fallas previsibles: extraer por tamaño de archivo en vez de por frontera de estado, o
mover mil líneas a un archivo nuevo que sigue recibiendo `&mut Midway` y sigue devolviendo
`Task<Message>` — un monolito con dos nombres (prohibido por el Req 14.3).

## Decisión

### 1. La unidad de extracción es el grupo de variantes del `Message` raíz

No el tamaño del archivo, no la carpeta, no la pantalla. Un grupo del `Message` raíz define
una frontera de responsabilidad que ya existe en el código y que el compilador puede
verificar.

### 2. Una extracción es válida solo si cumple las seis condiciones del patrón

Una extracción cuenta como vertical únicamente si el módulo destino:

1. Define su propio struct de estado, con los campos que la vertical posee, y `Midway`
   pasa a tener un campo de ese tipo en vez de campos sueltos.
2. Define su propio enum de mensajes en el módulo de la vertical. El `Message` raíz lo
   **envuelve**; no lo define.
3. Expone un `update` sobre su estado propio que devuelve una lista de Evento_Ascendente.
   No devuelve `Task<Message>` y no ejecuta efectos.
4. Expone un `view` que recibe sus dependencias por parámetro (estado propio +
   `DesignSystem`), sin `&Midway`, sin disco, sin SQL, sin reqwest y sin mutar nada.
5. No importa `AppState`, `crate::session`, `reqwest` ni `midway_core::infra`.
6. Tiene sus tests dentro del módulo, de transición de estado y de eventos emitidos,
   ejecutables sin ventana.

Mover un handler a otro archivo conservando la firma `(&mut Midway) -> Task<Message>`
**no** cuenta como extracción. Es un corte de vista, y ya hay diez en `src/ui/`.

### 3. Módulo destino por grupo

| Grupo | Módulo destino propuesto | Estado hoy |
| --- | --- | --- |
| `Theme` | `ui/theme_settings.rs` | Vertical de este spec |
| `Palette` | `ui/command_palette.rs` (ya existe, 122 líneas de overlay; el scoring vive en `src/command_palette.rs`, 321 líneas) | Handler en `app.rs` |
| `Workspace` (solo environments) | `ui/environment_settings.rs` | Handler en `app.rs`, mezclado con export/import, historial y carga de snapshot |
| `Workspace` (solo historial) | `ui/history_panel.rs` | Handler en `app.rs` |
| `Workspace` (export/import) | `workspace_interop.rs` | Handler en `app.rs`, 344 líneas con las `async` |
| `RequestComposer` (ciclo de vida de ejecución) | `request_execution.rs` | Handler en `app.rs`, 204 líneas |
| `RequestComposer` (persistencia de request) | `request_persistence.rs` | Handler en `app.rs`, 296 líneas |
| `RequestComposer` (edición del draft) | `ui/request_composer.rs` (ya existe, hoy solo vista) | Handler en `app.rs` |
| `ResponseInspector` | `ui/response_inspector.rs` (ya existe, hoy solo vista) | Handler en `app.rs` |
| `Tree` | `ui/request_tree_pane.rs` (ya existe) | Handler movido, frontera pendiente |
| `WorkspaceCrud` | `ui/workspace_crud.rs` | Handler en `app.rs` |
| `ActivityBar` | `ui/activity_bar.rs` (ya existe, hoy solo vista) | Handler en `app.rs` |
| `TopBar` | `ui/top_bar.rs` (ya existe, hoy solo vista) | Handler en `app.rs` |
| `PanelResize` | `ui/panel_layout.rs` | Handler en `app.rs` |
| `Keyboard` | queda en la raíz | Despacha a otros grupos: no es vertical |
| `Runner` | `collection_runner.rs` (ya existe) | Handler en `app.rs` |
| `Session` | `session.rs` (ya existe) | Handler en `app.rs` |
| `Updater` | `updater.rs` (ya existe) | Sin handler: cablear antes de extraer |
| `Tick` | — | Sin grupo propio: se resuelve al cablear `Session`/`Updater` |

`Workspace` se marca explícitamente como **grupo que hay que partir antes de extraer**: sus
19 variantes cubren cuatro asuntos sin relación entre sí, medidos por separado —
environments (161 líneas), historial (10), export/import (344) y carga de snapshot del
workspace (43), más 114 líneas del router `update_workspace`. Extraerlo como un bloque
produciría un archivo de 672 líneas con cuatro responsabilidades, es decir, un monolito más
chico. Lo mismo aplica a `RequestComposer` con sus 51 variantes: ciclo de vida de ejecución
(204), persistencia de request (296) y el resto en el router `update_request_composer` (527).

`Keyboard` (81 líneas) queda fuera de la fila por una razón distinta: su handler traduce
atajos a acciones de otros grupos. Extraerlo como vertical crearía una dependencia de
`Keyboard` hacia cada grupo. Su lugar es la raíz, como traductor de entrada.

### 4. Orden de prioridad

De menor a mayor acoplamiento con infraestructura, que es también de menor a mayor riesgo.
Este es el orden que fija el Req 8.11.

1. **`Theme`** — hecho en este spec. Establece el patrón.
2. **Selección de environment** — subconjunto de `Workspace` (161 líneas de environments)
   más `RequestComposerMessage::EnvironmentChanged` (`app.rs:161`). Estado acotado, lectura
   de `workspace.environments`, sin efecto asíncrono en el camino de selección.
3. **Paleta de comandos** — 116 líneas; depende de `workspace.collections` para construir
   ítems y de la apertura de tabs para ejecutarlos. Su Evento_Ascendente es "abrí este
   request", que es exactamente la clase de evento que el patrón necesita ejercitar a
   continuación.
4. **Historial** — subconjunto de `Workspace`, 10 líneas de handler más la carga asíncrona
   de lectura, sin mutación del workspace.
5. **Ciclo de vida de ejecución de request** — último. 204 líneas propias dentro de un
   grupo de 1 174. Toca `AppState`, canales de progreso, cancelación, eviction de bodies
   inactivos y persistencia, y es el único que exige diseñar la propiedad del estado de
   tabs.

Los grupos que **no** entran en la fila hasta que se resuelva un prerrequisito:

- `Updater` y `Tick`: primero hay que cablearlos al runtime (hoy son scaffolding con
  `#[allow(dead_code)]`). Extraer código muerto no produce evidencia.
- `Tree` y `WorkspaceCrud`: `TreeMessage` y `WorkspaceCrudMessage` comparten el estado del
  workspace y la reconciliación posterior a cada mutación
  (`reconcile_workspace_after_crud`, 94 líneas). Se tratan juntos, después de que la
  propiedad del cache de workspace tenga dueño.
- `PanelResize`: este mismo spec lo modifica en el incremento de UX. Extraer y modificar a
  la vez mezcla dos clases de riesgo.
- `Keyboard`: no es vertical, es traductor de entrada. Se queda en la raíz.

### 5. Restricciones que gobiernan toda extracción futura

- **Una vertical por cambio revisable.** No hay extracciones en lote.
- **El test de caracterización se escribe antes de extraer**, sobre el código sin
  modificar, y se adapta a la ruta nueva sin cambiar ningún valor esperado.
- **Sin trait común de vertical** hasta que existan al menos tres implementaciones reales.
  Con una sola, un trait es especulación.
- **Sin archivo intermedio que reencapsule el monolito** (Req 14.3). Si el módulo nuevo
  recibe `&mut Midway`, la extracción está mal hecha.
- **Sin cambio de esquema de persistencia como parte de una extracción.** Si una extracción
  parece requerirlo, se detiene y vuelve a diseño bajo el Req 11.5 completo.
- **Cada extracción registra su recuento**: líneas removidas de `app.rs`, tamaño del módulo
  nuevo y confirmación de comportamiento observable idéntico.
- El límite de 1 000 líneas aplica al módulo nuevo. Si `app.rs` sigue por encima de 1 000
  líneas al cierre de un spec, eso se justifica en un ADR (Req 7.6, 11.9).

### 6. Estado final buscado

`app.rs` como **raíz de composición**: dueña del `Message` raíz, del struct `Midway`, del
enrutamiento de `update`, de la composición de `view`, de `subscription`, de la traducción
de atajos de teclado y de la traducción de Eventos_Ascendentes a efectos. Nada más. Las
2 830 líneas de handlers atribuidas a grupos se van a su vertical.

Este ADR **no** fija fechas ni promete que las cinco verticales se hagan. Fija el criterio
y el orden, para que cada spec futuro tome el siguiente elemento de la fila en vez de
volver a discutir el criterio.

## Consecuencias

- Cada spec futuro que quiera tocar `app.rs` tiene un siguiente paso definido y una
  definición verificable de "bien hecho". La discusión pasa de "cómo dividimos esto" a
  "hacemos el siguiente de la fila".
- Las seis condiciones del punto 2 son verificables casi todas por el compilador y por un
  test de guardia de importaciones. La revisión no depende de criterio subjetivo.
- La atribución de líneas es reproducible, no estimada: `attribute_app_rs.py` falla si
  aparece una función de producción sin clasificar. Cuando `app.rs` cambie, la tabla se
  vuelve a medir en vez de re-adivinarse.
- El plan expone que dos de los quince grupos (`RequestComposer` y `Workspace`) **no son
  unidades de extracción**: hay que partirlos primero. Eso es trabajo de diseño que este
  ADR no hace, y reconocerlo evita una extracción mecánica que produciría monolitos más
  chicos.
- El plan expone que tres grupos (`Updater`, `Tree`, `Tick`) tienen prerrequisitos ajenos a
  la descomposición, y que uno (`Keyboard`) no es vertical en absoluto. Sin este registro,
  alguien podría "extraer" `Updater` en una tarde y no obtener nada verificable.
- El progreso es lento por diseño: una vertical por cambio revisable. A cambio, cada paso
  es reversible y está protegido por un test de caracterización escrito antes del cambio.
- Los diez módulos de `src/ui/` que hoy reciben `&Midway` quedan clasificados como cortes
  de vista, no como verticales. Convertirlos es trabajo pendiente, no trabajo hecho, y la
  Matriz_Funcionalidades no debe leerlos como fronteras logradas.
- Al terminar la fila completa, `app.rs` bajaría de sus ~5 610 líneas de producción al
  orden de 2 780. Es una cota derivada de la atribución por grupo, no una promesa: la
  medición real se registra extracción por extracción en `docs/product-audit.md`.
- Cambiar la unidad de extracción, el orden de prioridad o alguna de las seis condiciones
  requiere un ADR nuevo que supersede a este.
