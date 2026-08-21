# ADR 0006 — Justificación del tamaño de los archivos que permanecen sobre 1 000 líneas al cierre

- **Estado**: Aceptado
- **Fecha**: 2026-08-21
- **Requisitos**: 7.6 (todo archivo que permanece sobre 1 000 líneas al cierre) y 11.9 (todo
  archivo **tocado** por este spec que supera 1 000 líneas). Relacionados: 5.1, 5.2 (registro
  del inventario), 8.8 (el módulo de la vertical queda por debajo del límite), 8.9 (reducción
  de `app.rs`)
- **Evidencia**: medición del árbol de trabajo al cierre del spec, hecha por esta tarea
  (15.3), no copiada de la tarea 8.2. Método y comandos exactos en la sección 4. Contraste
  contra la línea base `1fc270326e5c304f24ce08fe1f528da33a007d3e` obtenido con `git show`,
  que es de solo lectura (Req 2.1-2.3)
- **Principio de origen**: P9 de `docs/product-principles.md` — "un archivo grande necesita
  defensa escrita". Este documento es esa defensa; no es una autorización para crecer

## Contexto

### 1. Qué se midió y cuándo

La tarea 8.2 midió `app.rs` **después de la tarea 7.3** y registró el resultado en
`docs/product-audit.md` §12: 11 390 líneas crudas. Ese número ya no es el vigente. Entre esa
medición y el cierre, las tareas 10 (divisor horizontal del panel de respuesta) y 11 (estados
vacíos, jerarquía visual e idioma) agregaron código de producción y tests a `app.rs` y a tres
archivos más. Esta sección mide de nuevo, al cierre, y todos los números de este ADR son de
esa medición.

Se registran **dos recuentos por archivo**, con el mismo criterio que la §5 y la §12 de
`docs/product-audit.md`:

- **Crudo**: `wc -l` del archivo completo. Es el número que dispara el umbral de 1 000 líneas
  de los Req 7.6 y 11.9.
- **Producción**: el archivo menos las líneas alojadas en bloques `#[cfg(test)]`. Es la
  superficie a descomponer.

La distinción no es cosmética: cuatro de los ocho archivos de la tabla tienen **más líneas de
test que de producción**, y en dos de ellos la producción está por debajo de 500 líneas. Un
umbral aplicado al recuento crudo clasifica como monolito a un archivo cuya causa de tamaño es
su propia cobertura.

### 2. Los ocho archivos sobre 1 000 líneas al cierre

Al cierre son **ocho**, no siete: uno cruzó el umbral durante este spec. La columna "Tocado
por este spec" es la que decide si aplica el Req 11.9 además del 7.6.

| Archivo | Crudo | Producción | Test | Línea base (crudo) | Tocado por este spec | Requisito que lo trae acá |
| --- | --- | --- | --- | --- | --- | --- |
| `midway-desktop/src/app.rs` | **12 678** | 5 963 | 6 715 | 10 750 | **Sí** (tareas 4.1, 4.4, 4.5, 4.6, 7.1, 7.2, 7.3, 10.1, 10.2, 10.3, 10.4, 10.5, 11.2, 11.3) | 7.6 y **11.9** |
| `midway-core/src/infra/sqlite_repository.rs` | 2 322 | 1 485 | 837 | 2 322 | No: byte a byte idéntico a la línea base | 7.6 |
| `midway-desktop/src/updater.rs` | 2 311 | 1 003 | 1 308 | 2 311 | No: byte a byte idéntico a la línea base | 7.6 |
| `midway-desktop/src/ui/request_tree_pane.rs` | **2 118** | 1 339 | 779 | 2 115 | **Sí** (tareas 7.1 y 11.3) | 7.6 y **11.9** |
| `midway-core/src/domain/interop.rs` | 2 068 | 1 535 | 533 | 2 068 | No: byte a byte idéntico a la línea base | 7.6 |
| `midway-desktop/src/collection_runner.rs` | 1 497 | 427 | 1 070 | 1 497 | No: byte a byte idéntico a la línea base | 7.6 |
| `midway-desktop/src/session.rs` | **1 300** | 298 | 1 002 | 1 054 | **Sí** (tareas 4.2 y 4.3, solo `#[cfg(test)]`) | 7.6 y **11.9** |
| `midway-desktop/src/ui/request_composer.rs` | **1 103** | 929 | 174 | 926 | **Sí** (tarea 11.3) | **11.9** (y 7.6). **Cruzó el umbral en este spec** |

Verificación de la columna "Tocado": `git diff --quiet <baseline> -- <archivo>` sale con 0
para los cuatro archivos declarados idénticos. No es una afirmación de memoria.

Total del workspace bajo `midway-core/src` y `midway-desktop/src`: **34 996 líneas en 47
archivos**, contra 31 786 en 45 en la línea base (+3 210 líneas, +2 archivos: los módulos
nuevos `ui/theme_settings.rs` y `ui/empty_state.rs`). El sistema, medido en líneas, es más
grande que antes de este spec. Eso se registra, no se maquilla, y se desglosa: de las +3 210,
**+2 427 están dentro de bloques `#[cfg(test)]`** y +783 son producción. Fuera de `src/` hay
además un archivo de test nuevo, `midway-desktop/tests/vertical_import_guard.rs` (314 líneas),
que no entra en estos totales.

### 3. Los archivos que **no** cruzaron el umbral

Registrado porque el Req 8.8 lo exige para el módulo de la vertical y porque el margen del
archivo más cercano al límite es información operativa:

| Archivo | Crudo | Producción | Nota |
| --- | --- | --- | --- |
| `midway-desktop/src/ui/theme_settings.rs` (nuevo, tarea 6.1) | 204 | 129 | Req 8.8 cumplido con 796 líneas de margen. Nada que justificar acá |
| `midway-desktop/src/ui/empty_state.rs` (nuevo, tarea 11.1) | 432 | 224 | Bajo el límite con 568 líneas de margen |
| `midway-desktop/src/curl.rs` | 927 | 701 | **No tocado por este spec** (idéntico byte a byte a la línea base), pero es el archivo no listado más cercano al umbral: 73 líneas de margen. El próximo que lo cruce sin plan es este |
| `midway-desktop/src/ui/response_inspector.rs` | 717 | 471 | Tocado por las tareas 11.2 y 11.3; sigue bajo el límite |
| `midway-desktop/src/ui/top_bar.rs` | 537 | 298 | Tocado por la tarea 7.2; su producción **bajó** 12 líneas |

### 4. Método, reproducible

```bash
# Recuento crudo de todo el fuente, ordenado
find midway-core/src midway-desktop/src -name '*.rs' -exec wc -l {} + | sort -rn | awk '$1 > 900'

# Separación producción / test con el criterio de la tarea 8.2
python3 docs/baseline-evidence/count_test_lines.py midway-desktop/src/app.rs <resto de archivos>

# Delta contra la línea base, con diff auditable de las líneas de producción
git show 1fc270326e5c304f24ce08fe1f528da33a007d3e:midway-desktop/src/app.rs > /tmp/app_baseline.rs
python3 docs/baseline-evidence/prod_line_delta.py /tmp/app_baseline.rs midway-desktop/src/app.rs

# Confirmación de que un archivo no fue tocado
git diff --quiet 1fc270326e5c304f24ce08fe1f528da33a007d3e -- midway-core/src/infra/sqlite_repository.rs
```

**Precisión de método, porque hay dos juegos de números y ninguno se esconde.** El contador de
llaves de `count_test_lines.py` y `prod_line_delta.py` es ingenuo: no distingue llaves dentro
de literales ni de comentarios, así que fusiona bloques `#[cfg(test)]` contiguos y absorbe las
líneas separadoras entre ellos. Sobre el `app.rs` de hoy reporta `test=6726 prod=5952` con 5
bloques; el conteo exacto —anotación `#[cfg(test)]` hasta la primera llave de cierre en
columna cero, ambas inclusive— da **11 bloques, `test=6715 prod=5963`**. La diferencia es de
11 líneas, todas separadoras entre bloques, y ninguna conclusión de este ADR depende de ellas.
Las tablas de este documento usan el conteo exacto; los deltas citados desde
`prod_line_delta.py` usan el del script, que es el mismo criterio en el antes y en el después
y por lo tanto es válido como delta. Es la misma clase de discrepancia que el ADR 0004 ya
registró para la línea base (5 132/5 618 exacto contra 5 140/5 610 del script).

### 5. `app.rs`: de dónde salen las 1 928 líneas nuevas

| Recuento | Línea base | Cierre | Delta |
| --- | --- | --- | --- |
| Crudo | 10 750 | **12 678** | **+1 928** |
| Producción | 5 618 | **5 963** | **+345** |
| Test (`#[cfg(test)]`) | 5 132 | 6 715 | +1 583 |
| Producción sin comentarios ni líneas en blanco | 3 689 | 3 831 | +142 |

Los bloques `#[cfg(test)]` pasaron de 8 a 11. Atribución completa del +1 583, que cierra
exacto:

| Módulo de test | Línea base | Cierre | Delta | Tarea que lo agrandó |
| --- | --- | --- | --- | --- |
| `mod panel_resize_tests` | 135 | 1 168 | **+1 033** | 4.6 (límites vigentes), 10.4 (Property 5), 10.5 (Property 6) |
| `mod debug_layout_tests` | 20 | 166 | +146 | 4.5 (Property 4) |
| `mod session_restore_property_tests` | — | 168 | +168 | 4.4 (Property 3) |
| `mod theme_toggle_characterization_tests` | — | 127 | +127 | 4.1, adaptado en 7.3 |
| `mod run_progress_label_tests` | — | 109 | +109 | 11.3 (idioma de la línea de progreso) |
| `mod import_name_collision_tests` | 52 | 52 | 0 | — |
| `mod tests` | 4 158 | 4 158 | 0 | — |
| `mod error_boundary_tests` | 269 | 269 | 0 | — |
| `mod resolve_startup_collection_tests` | 101 | 101 | 0 | — |
| `mod navigation_toggle_preserves_composer_state_tests` | 224 | 224 | 0 | — |
| `mod collection_selection_workspace_property_tests` | 173 | 173 | 0 | — |
| **Total** | **5 132** | **6 715** | **+1 583** | |

**El 82 % del crecimiento crudo de `app.rs` son tests.** No es deuda y no se descuenta de
nada: es el efecto buscado de la Compuerta de caracterización (tareas 4.x) y de las Properties
5 y 6 (tareas 10.4 y 10.5). `mod panel_resize_tests` es hoy el bloque de test más grande
después de `mod tests` porque cinco funciones de aritmética de paneles quedaron cubiertas por
propiedad con entradas degeneradas (cero, negativos, `NaN`, infinitos).

Atribución del +345 de producción, sumada sobre el diff que emite `prod_line_delta.py` (341
por el método del script; los 4 restantes son la diferencia de método de la sección 4):

| Origen | Delta de producción | Detalle |
| --- | --- | --- |
| Tarea 10 — divisor horizontal del panel de respuesta | **+255** | 15 de constantes (`RESPONSE_PANE_MIN_HEIGHT`, `RESPONSE_PANE_MAX_HEIGHT`, `RESPONSE_DIVIDER_HIT_HEIGHT`, `REQUEST_PANE_MIN_HEIGHT` con su doc), 59 de las dos funciones puras de altura, 43 netas de `PanelDragState::ResponseHeight` y `PanelResizeMessage` (la mayor parte es el doc de por qué `PartialEq` se implementa a mano), 48 de `horizontal_resize_divider`, 48 de `divider_drag_started_message` **menos 17** que ese mismo refactor removió de `vertical_resize_divider`, 12 del envoltorio `responsive` de `view`, y el resto en enrutamiento del eje y propagación de `window_height` |
| Tarea 11 — estados vacíos, jerarquía visual e idioma | **+55** | 40 de `run_progress_label` (reemplaza un `format!("{progress:?}")` visible al usuario), 5 del encabezado en español, 5 del borde del inspector, 1 neto del estado vacío "Sin requests abiertos" (que **removió** 8 líneas de un `container` armado a mano), y el resto en imports |
| Hunk compartido entre 10.3 y 11.3 | +25 | La firma de `debug_split_content` gana dos parámetros para el divisor y a la vez recibe los tres niveles de elevación; el doc que explica por qué el inspector lleva `Border` cerrado y el explorador una franja no se puede atribuir a una sola de las dos tareas |
| Tarea 7 — vertical Tema/Ajustes | **+6** | Ya medido y desglosado línea por línea en `docs/product-audit.md` §12.4 |
| **Total** | **+341** | Suma exacta del diff de producción que emite `prod_line_delta.py` |

Lectura honesta: **la única tarea de este spec cuyo objetivo declarado era reducir `app.rs`
aportó +6 líneas, y las tareas de funcionalidad aportaron +335.** Las de funcionalidad no
prometieron reducir nada, así que no incumplen; la de extracción sí, y esa brecha es el punto
siguiente.

### 6. La brecha del Req 8.9, sin atenuantes

El Req 8.9 pide que `app.rs` **reduzca** su recuento respecto de las 10 750 de la línea base.
No lo hizo. `docs/product-audit.md` §12.8 ya lo registró como brecha con Hito 2 propietario, y
este ADR confirma el número **al cierre**, que es peor que el de la tarea 8.2:

| Momento | Crudo | Producción |
| --- | --- | --- |
| Línea base | 10 750 | 5 618 |
| Después de la tarea 7.3 (medición de la tarea 8.2) | 11 390 | 5 617 |
| **Al cierre del spec (esta medición)** | **12 678** | **5 963** |

La extracción de la vertical Tema/Ajustes movió la lógica de tema fuera de `app.rs` —el
archivo ya no define `ThemeMessage` ni contiene `update_theme`— y aun así el archivo creció.
El costo de la frontera explícita (importar el módulo, reexportar el mensaje, traducir el
Evento_Ascendente y documentar por qué) fue del mismo orden que el código removido. En una
vertical de una sola variante de mensaje, ese costo se come la reducción entera.

Consecuencia que este ADR fija como regla: **el recuento de líneas de `app.rs` no sirve como
métrica de progreso de la descomposición.** Lo que sí sirve es cuántos grupos del `Message`
raíz tienen frontera verificable, que hoy es **uno de quince**.

### 7. Un dato de proceso que salió de esta medición

`docs/baseline-evidence/verify_adr_citations.py` codifica 61 citas `archivo:línea` de los ADRs
0003 y 0004 contra los números de línea de la línea base. Ejecutado hoy: **55 de 61 fallan**,
porque `app.rs` creció 1 928 líneas y todo se corrió de lugar. No hay ninguna afirmación
incorrecta en esos ADRs: lo que caducó son los punteros, no los hechos. Se registra acá porque
es la primera consecuencia concreta de que un archivo de 12 678 líneas es difícil de citar de
forma estable, y porque este ADR **evita el problema citando por nombre de símbolo y no por
número de línea**. Reparar el verificador es trabajo del Hito 2, junto con la descomposición
que va a mover esas líneas otra vez.

## Decisión

### 1. Los ocho archivos permanecen sobre 1 000 líneas al cierre. Ninguno se parte en este spec

Partir cualquiera de ellos ahora significaría hacerlo sin el plan del ADR 0004 (que exige una
vertical por cambio revisable, con test de caracterización escrito antes) o sin el
prerrequisito que cada archivo tiene. Un corte apurado produce el resultado que el Req 14.3
prohíbe: un archivo nuevo que sigue recibiendo `&mut Midway` y sigue devolviendo
`Task<Message>`, o sea un monolito con dos nombres.

### 2. Justificación y hito propietario, uno por archivo

#### 2.1 `midway-desktop/src/app.rs` — 12 678 crudas / 5 963 de producción

**Tocado por este spec. Req 11.9 aplica.**

- **Por qué sigue sobre el límite.** Es la raíz de composición de una app `iced`: dueña del
  `Message` raíz (15 variantes), de los catorce enums de mensajes por área, del struct
  `Midway`, de los handlers de `update`, de `view`, de `subscription` y de la aritmética de
  paneles. El ADR 0004 midió que **2 830** de sus líneas de producción son handlers
  atribuibles a grupos de mensajes y por lo tanto extraíbles, y que el resto (~2 780) es
  composición legítima de la raíz. Descomponer eso son cinco extracciones sucesivas, cada una
  con su test de caracterización, y dos grupos (`RequestComposer` con 51 variantes, `Workspace`
  con 19 y cuatro asuntos sin relación) que **hay que partir antes de poder extraer**. No cabe
  en el alcance de un spec cuyo objetivo era auditar el baseline y ejercitar el patrón una vez.
- **Por qué creció en vez de encogerse.** Sección 5: +1 583 de tests (82 % del delta crudo),
  +345 de producción de los cuales +335 son funcionalidad nueva pedida por los Req 9.2, 9.3,
  9.5, 9.6 y 9.1, y +6 el costo neto de la frontera de la vertical.
- **Qué sí quedó garantizado.** Un grupo de quince tiene frontera con contrato verificable:
  `update` total sin `Result`, sin `Task` y sin I/O; sin importar `AppState`, `crate::session`,
  `reqwest` ni `midway_core::infra` (fijado por
  `midway-desktop/tests/vertical_import_guard.rs`); comportamiento observable idéntico
  (`mod theme_toggle_characterization_tests`). Y `guarded_update` / `guarded_view` acotan el
  pánico por área en runtime, lo que limita el daño del tamaño sin reducirlo.
- **Hito propietario**: **Hito 2**, con el plan completo en
  `docs/adr/0004-descomposicion-de-app-rs.md` (unidad de extracción, seis condiciones de
  validez, módulo destino por grupo y orden de prioridad). Limitación `L8` de
  `docs/known-limitations.md`, cuyo recuento de 10 750 queda desactualizado por este ADR: el
  vigente es 12 678. Lo mismo vale para `L9`, que enumera **seis** archivos con sus recuentos de
  línea base: al cierre son **siete** (se suma `request_composer.rs`) y tres de esos recuentos
  cambiaron. Actualizar esas dos filas no es alcance de esta tarea, que solo crea este ADR; la
  tabla de la sección 2 de acá es el registro vigente.
- **Criterio de cierre de la brecha**: `app.rs` queda como raíz de composición, del orden de
  2 800 líneas de producción, cuando los grupos de la fila del ADR 0004 estén extraídos. Es una
  cota derivada de la atribución por grupo, no una promesa de fecha.

#### 2.2 `midway-core/src/infra/sqlite_repository.rs` — 2 322 crudas / 1 485 de producción

**No tocado por este spec** (idéntico byte a byte a la línea base). Req 11.9 no aplica; entra
por el Req 7.6.

- **Por qué sigue sobre el límite.** Concentra toda la persistencia: `migrate`, workspace,
  colecciones, requests, environments, historial y metadata de secretos. Partirlo por entidad
  es viable, pero el problema dominante de este archivo **no es su tamaño**: es que `migrate()`
  son `CREATE TABLE IF NOT EXISTS` más `ALTER TABLE` condicional por `PRAGMA table_info`, sin
  `user_version`. Sin versión de esquema no hay migración verificable, ni respaldo, ni
  rollback. Partir el archivo antes de resolver eso mueve el problema a más lugares.
- **Hito propietario**: **Hito 6** para el esquema (limitación `L25`), **Hito 2** para el
  tamaño, y en ese orden. Concentra además 97 de las 358 ocurrencias de `unwrap()`/`expect()`
  del workspace (`L21`, Hito 2).

#### 2.3 `midway-desktop/src/updater.rs` — 2 311 crudas / 1 003 de producción

**No tocado por este spec** (idéntico byte a byte a la línea base).

- **Por qué sigue sobre el límite.** Tiene 1 003 líneas de producción que **nunca se ejecutan**:
  el módulo entero lleva `#![allow(dead_code)]` y `Message::Updater(_)` devuelve `Task::none()`.
  Reducir un archivo cuyo código no tiene llamador no produce ninguna evidencia: no hay test de
  caracterización posible sobre comportamiento que no ocurre, así que un refactor acá sería
  irreversible sin red. El orden correcto es cablearlo primero y recortarlo después, si al
  cablearlo sigue haciendo falta todo.
- **Dato de forma**: 1 308 de sus 2 311 líneas son test. El archivo está más cubierto que
  ejecutado.
- **Hito propietario**: **Hito 4** (cableado, limitación `L10`). El tamaño se revisa después,
  bajo el Hito 2.

#### 2.4 `midway-desktop/src/ui/request_tree_pane.rs` — 2 118 crudas / 1 339 de producción

**Tocado por este spec. Req 11.9 aplica.** Delta: +3 crudas, +3 de producción.

- **Qué cambió acá.** Nada de tamaño: la tarea 7.1 cambió sus puntos de lectura de tema a
  `state.theme.mode()`, y la tarea 11.3 reemplazó un `container::Style::border` uniforme por
  `pane_shell`, una franja de 1 px de un solo lado, porque `iced::Border` es uniforme en los
  cuatro lados y encajonaba el panel. Ese cambio agregó 55 líneas de producción y removió 52:
  neto +3, con la mayor parte de lo agregado en el doc que explica por qué la franja y no el
  borde.
- **Por qué sigue sobre el límite.** Es el único módulo de `src/ui/` con handler de `update`
  propio (`update_tree`), pero recibe `&mut Midway` completo y devuelve `Task<Message>`, y
  `TreeMessage` sigue definido en `app.rs`. Es un **archivo movido, no una frontera**: tiene la
  apariencia de módulo extraído sin ninguna de sus garantías. Convertirlo en vertical exige
  antes decidir la propiedad del cache de workspace, porque `TreeMessage` y
  `WorkspaceCrudMessage` comparten ese estado y la reconciliación posterior a cada mutación
  (`reconcile_workspace_after_crud`). El ADR 0004 lo pone explícitamente **fuera de la fila**
  hasta que ese prerrequisito se resuelva.
- **Hito propietario**: **Hito 2**, tratado junto con `WorkspaceCrud` (limitación `L9`).

#### 2.5 `midway-core/src/domain/interop.rs` — 2 068 crudas / 1 535 de producción

**No tocado por este spec** (idéntico byte a byte a la línea base).

- **Por qué sigue sobre el límite.** Import y export de tres formatos (bundle nativo v1,
  Postman Collection v2.1, OpenAPI v3 JSON/YAML) en un solo módulo, sin separación por formato.
  El corte es obvio —un submódulo por formato— y de bajo riesgo, pero es dominio puro y no toca
  ningún objetivo de este spec: incluirlo habría mezclado un refactor de `midway-core` con la
  extracción de una vertical de `midway-desktop`, y una regresión no se habría podido atribuir
  a una causa.
- **Riesgo aceptado mientras tanto**: un formato nuevo agranda el mismo archivo, y un bug de
  parseo de un formato se revisa junto al código de los otros dos.
- **Hito propietario**: **Hito 2** (limitación `L9`). Es el candidato de mejor relación
  beneficio/riesgo de los cuatro no tocados.

#### 2.6 `midway-desktop/src/collection_runner.rs` — 1 497 crudas / 427 de producción

**No tocado por este spec** (idéntico byte a byte a la línea base).

- **Por qué sigue sobre el límite.** Su producción son **427 líneas**, muy por debajo del
  umbral; el archivo cruza las 1 000 por sus **1 070 líneas de test**. Aplicar el umbral a este
  archivo como si fuera un monolito sería un error de lectura: no hay nada que descomponer.
- **El problema real de este archivo es otro**: el runner funciona pero no es disparable ni
  cancelable desde la UI (`RunnerMessage::StartRequested` y `CancelRequested` con
  `#[allow(dead_code)]`, sin botón "Run"). Funcionalidad completa e inalcanzable.
- **Hito propietario**: **Hito 4** (cableado, limitación `L12`). **No se declara ninguna acción
  de tamaño**: 427 líneas de producción no la justifican.

#### 2.7 `midway-desktop/src/session.rs` — 1 300 crudas / 298 de producción

**Tocado por este spec. Req 11.9 aplica.** Delta: +246 crudas, **+0 de producción**.

- **Qué cambió acá.** Exclusivamente `#[cfg(test)]`: la tarea 4.2 agregó
  `mod session_snapshot_serde_round_trip_property_tests` (Property 2, ida y vuelta de
  `SessionSnapshot`) y la tarea 4.3 agregó `mod session_schema_characterization_tests` (la clave
  JSON `panelSizes.responsePanelHeight`, la deserialización sin campos opcionales y el tripwire
  `SESSION_SCHEMA_VERSION == 1`). El diff de producción es **literalmente vacío**: 0 líneas
  agregadas, 0 removidas, verificado con `prod_line_delta.py`.
- **Por qué sigue sobre el límite.** Por sus tests: 1 002 de 1 300 líneas. La producción son
  298 líneas y no hay monolito que partir. El archivo creció porque este spec lo **cubrió**,
  que es lo que el Req 6.3 pedía.
- **El problema real de este archivo es otro**: `SESSION_SCHEMA_VERSION = 1` sin ruta de
  migración. Un `session.json` con otra versión se descarta entero, y el usuario pierde tabs
  abiertas, tab activa, tamaños de panel y stack de tabs cerradas. El tripwire de la tarea 4.3
  ahora **falla en voz alta** si alguien cambia el esquema sin declararlo, que es la garantía
  que faltaba.
- **Hito propietario**: **Hito 6** (esquema versionado y migración, limitación `L26`). **No se
  declara ninguna acción de tamaño.**

#### 2.8 `midway-desktop/src/ui/request_composer.rs` — 1 103 crudas / 929 de producción

**Tocado por este spec. Req 11.9 aplica. Este archivo cruzó el umbral durante este spec: 926 →
1 103.** Es el hecho menos cómodo de este ADR y se declara primero.

- **Qué lo cruzó.** La tarea 11.3, unificación de idioma (Req 9.6): +152 líneas de test
  (`mod language_tests`, ocho tests que fijan qué se traduce y qué se conserva, incluido
  `retired_english_labels_do_not_come_back`) y **+25 de producción**, de las cuales la mayoría
  son doc que explica por qué `Bearer`, `Basic`, `API Key`, `JSON` y `Form data` se conservan en
  inglés —son nombres canónicos de esquemas y formatos del protocolo, excepción registrada como
  `L43` en `docs/known-limitations.md`— mientras `None`, `Text`, `Send`, `Key` y `Value` pasan a
  español. El cambio funcional son cadenas visibles y una constante (`SIN_ENVIRONMENT_LABEL`).
- **Por qué se acepta el cruce en vez de evitarlo.** El recuento de **producción es 929**: sigue
  por debajo de 1 000. Lo que cruzó el umbral crudo son los tests que fijan la traducción, y la
  alternativa —traducir cadenas visibles sin dejar tests que impidan que el inglés vuelva— es
  peor: el Req 9.6 quedaría cumplido hoy y sin defensa mañana. Mover esos tests a un archivo
  aparte para que el recuento del módulo no cruce el umbral sería contabilidad creativa, no
  arquitectura: el patrón del proyecto es tests junto al código que prueban.
- **Por qué la producción está donde está.** 929 líneas de vista del editor de request: método,
  URL, headers, params, body con cuatro modos, autenticación con cuatro esquemas, assertions y
  form-data. Es el corte de vista más grande de `src/ui/`, y su handler de `update` —1 174
  líneas— todavía vive en `app.rs`. El ADR 0004 lo tiene identificado: el grupo
  `RequestComposer` (51 variantes) **no es unidad de extracción** hasta partirlo en ciclo de
  vida de ejecución, persistencia de request y edición del draft; solo esa última parte va a
  este archivo.
- **Hito propietario**: **Hito 2**, dentro de la partición del grupo `RequestComposer`
  (`docs/adr/0004-descomposicion-de-app-rs.md`, punto 3).
- **Criterio de cierre**: cuando el grupo se parta, el handler de edición del draft llega acá y
  el archivo va a **crecer** antes de que la vista se pueda dividir por pestañas del composer.
  Ese aumento es esperado y no requiere un ADR nuevo; sí lo requiere cualquier decisión de
  dejarlo permanentemente sobre 1 000 líneas de producción.

### 3. Lo que este ADR **no** autoriza

- No autoriza que un archivo nuevo nazca sobre 1 000 líneas. El Req 8.8 sigue vigente y los dos
  módulos creados por este spec lo cumplen con amplio margen (204 y 432 líneas crudas).
- No autoriza usar este documento como precedente para el próximo archivo que cruce el umbral.
  `curl.rs` está a 73 líneas del límite; si lo cruza, necesita su propia entrada, no una
  extensión de esta.
- No autoriza tratar el umbral de 1 000 líneas como si midiera lo mismo en todos los casos. La
  sección 2 muestra que en tres de los ocho archivos (`collection_runner.rs`, `session.rs`,
  `updater.rs`) el recuento crudo está dominado por tests o por código sin llamador, y el
  problema que hay que arreglar en esos tres no es el tamaño.
- No autoriza leer el crecimiento de `app.rs` como fracaso del patrón de extracción ni como
  éxito. La medición dice una sola cosa: **el recuento de líneas no mide progreso de
  descomposición.** Lo que mide es cuántos grupos tienen frontera verificable: uno de quince.

### 4. Regla que queda vigente para las entregas siguientes

Al cierre de cada spec que toque el fuente:

1. Se vuelve a medir con los comandos de la sección 4, crudo y producción por separado.
2. Todo archivo tocado que supere 1 000 líneas entra en un ADR con las cuatro columnas de este:
   por qué sigue sobre el límite, qué lo hizo crecer, hito propietario y criterio de cierre
   (Req 11.9).
3. Si un archivo **baja** de 1 000 líneas, sale de la lista y eso también se registra: la lista
   tiene que poder encogerse, no solo crecer.
4. Un archivo cuyo recuento crudo cruza el umbral por cobertura de test se declara como tal y
   **no** genera acción de tamaño. Reducir tests para bajar de un umbral está prohibido.

## Consecuencias

- Los ocho archivos sobre 1 000 líneas quedan con justificación escrita, hito propietario y
  criterio de cierre. La pregunta "¿por qué sigue así?" tiene respuesta en un lugar, y la
  respuesta distingue los cuatro archivos que este spec tocó de los cuatro que heredó sin
  tocar. Esa distinción importa: sobre los heredados este spec no tiene nada que explicar más
  allá de no haberlos empeorado, y se verificó con `git diff --quiet` que no lo hizo.
- Queda registrado, en el mismo documento que justifica el tamaño, que **este spec empujó un
  archivo por encima del umbral** (`request_composer.rs`, 926 → 1 103). Es la clase de dato que
  un cierre optimista omite. Su producción sigue bajo el límite y la causa son tests que
  protegen el Req 9.6, pero el hecho se declara sin adorno.
- Queda registrado que `app.rs` **creció 1 928 líneas** durante un spec que tenía entre sus
  requisitos reducirlo, con la atribución completa de cada línea: 82 % tests, el resto
  funcionalidad pedida por los Req 9.x, y +6 el costo neto de la única extracción. La brecha
  del Req 8.9 queda confirmada al cierre, no solo en la medición intermedia de la tarea 8.2.
- El recuento de líneas queda **descartado como métrica de progreso** de la descomposición,
  con evidencia en vez de opinión. La métrica que reemplaza es contable y verificable por el
  compilador: cuántos de los quince grupos del `Message` raíz tienen frontera que cumpla las
  seis condiciones del ADR 0004. Hoy: uno.
- Tres archivos salen de la categoría "monolito" y entran en la categoría correcta:
  `collection_runner.rs` (427 líneas de producción, problema de cableado),
  `session.rs` (298, problema de esquema) y `updater.rs` (1 003 sin llamador, problema de
  cableado). Cualquier plan futuro que los agrupe con `app.rs` por su recuento crudo va a
  gastar esfuerzo en el lugar equivocado.
- El ADR queda desactualizado por construcción en cuanto alguien toque uno de estos archivos.
  Eso es intencional: la regla de la sección 4 de la decisión obliga a volver a medir en cada
  cierre, en vez de arrastrar los números de este documento como si fueran permanentes. Los
  números de la tarea 8.2 (`app.rs` = 11 390) ya quedaron obsoletos en cuatro tareas.
- Este ADR **no** cita `archivo:línea` en ningún punto, por diseño: 55 de las 61 citas que
  `verify_adr_citations.py` guarda para los ADRs 0003 y 0004 ya no coinciden, porque `app.rs`
  se corrió 1 928 líneas. Reparar ese verificador es trabajo del Hito 2 y no se hace acá.
- Cambiar el umbral de 1 000 líneas, la regla de medir crudo y producción por separado, o la
  decisión de no partir ninguno de estos ocho archivos en este spec, requiere un ADR nuevo que
  supersede a este.
