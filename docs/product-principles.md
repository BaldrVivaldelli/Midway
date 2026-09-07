# Principios de producto — Midway

> Documento normativo derivado de los requisitos, no descriptivo del baseline. Cada
> principio es una **restricción que gobierna las entregas**; el estado observado del
> baseline se registra al lado del principio para que la distancia entre la regla y la
> realidad quede visible en vez de implícita.
>
> Origen: las restricciones no negociables del producto (Requisito 11), las prohibiciones
> de entrega (Requisito 14) y la prohibición de imitación visual (Requisito 9.8) del spec
> `midway-baseline-audit-and-first-vertical`. Este documento **no crea reglas nuevas**:
> reformula esas restricciones como principios verificables y les asigna un método de
> verificación. Donde un principio no tiene verificación automatizada hoy, se dice.
>
> Alcance de este archivo al cierre de la tarea 2.5: los quince principios de las
> secciones 2 a 6 quedan escritos, con su requisito de origen, su método de verificación y
> su estado observado en el baseline (HEAD `1fc270326e5c304f24ce08fe1f528da33a007d3e`).
> Las tareas posteriores del spec **no reescriben este documento**: cuando una tarea cierra
> una brecha registrada acá, actualiza la columna "Estado observado" del principio
> correspondiente y deja anotada la tarea que lo hizo.
>
> Método: cada dato del baseline citado acá se midió con un comando de solo lectura sobre
> el árbol de trabajo en ese HEAD, o se toma de `docs/product-audit.md`,
> `docs/known-limitations.md` y `docs/architecture.md`, citando la sección exacta. Ningún
> número está estimado.
>
> **Afirmaciones que este documento no hace.** No se afirma "paridad total de
> funcionalidades", "100% compatible", "listo para producción", "verificado
> multiplataforma", "cero regresiones", "enteramente Rust", "no requiere Node" ni
> "migración completa" (Req 14.5). Donde una afirmación de ese tipo aparece en el
> repositorio, queda registrada como violación observada con su dueño, no repetida.

## 1. Cómo leer un principio

Cada principio tiene cuatro partes fijas:

| Parte | Qué contiene |
| --- | --- |
| **Enunciado** | La regla, en forma normativa. Es lo que se puede citar en una revisión para rechazar un cambio. |
| **Requisito de origen** | El criterio de aceptación del que se deriva. Si el enunciado y el criterio se contradicen, manda el criterio. |
| **Verificación** | Cómo se comprueba: comando reproducible, test automatizado, o revisión humana. Si es revisión humana, se dice. |
| **Estado observado** | Lo que hay hoy en el baseline: `Cumple`, `Cumple parcialmente`, `Brecha registrada` (con el `L…` de `docs/known-limitations.md` y su hito) o `No aplica todavía`. Cuando el principio no tiene barrera automatizada, se dice acá. |

`Cumple` significa **cumple según la verificación indicada**, no "está garantizado". Un
principio verificado solo por lectura del fuente no tiene barrera que impida romperlo en el
próximo commit; eso se dice en su fila de verificación.

## 2. Principios de plataforma y stack

### P1 — Un solo stack: Rust nativo, sin runtimes web ni ajenos

**Enunciado.** El workspace se mantiene libre de Tauri, Electron, WebView, React, Node.js,
Python, Qt, Flutter y JVM. Ninguna entrega introduce una dependencia de esas familias, ni
directa ni transitiva, ni como paso obligatorio de build del binario.

**Requisito de origen.** Req 11.1. Complementado por el Req 13.9 (el entregable funciona
sin Tauri, sin WebKit, sin Node.js y sin JavaScript) y por el Req 10.8 (CI libre de
Node.js, npm y de cualquier herramienta JavaScript, incluidas las de documentación y
reportes).

**Verificación.**

```bash
# Ausencia de la familia Tauri y de motores de WebView en el grafo resuelto
grep -c 'name = "tauri' Cargo.lock   # esperado: 0
grep -c 'name = "wry"' Cargo.lock    # esperado: 0
grep -c 'name = "tao"' Cargo.lock    # esperado: 0
grep -c 'name = "webkit2gtk' Cargo.lock  # esperado: 0

# Tooling JavaScript versionado
git ls-files '*.mjs' '*.ts' '*.tsx' 'package.json'
```

Barrera automatizada parcial: `midway-core/tests/no_tauri_dependency.rs` recorre el
subgrafo transitivo de `midway-core` sobre `resolve.nodes` de `cargo metadata` y falla si
aparece `tauri` o cualquier `tauri-*`. Su alcance es `midway-core`, **no** el workspace
completo, y su filtro es solo Tauri: no cubre `wry`, `tao`, WebKit, ni el resto de la
lista.

**Estado observado.** Cumple para el **artefacto**; brecha registrada para el **proyecto**.
Los cuatro `grep` sobre `Cargo.lock` devuelven 0: el binario no lleva WebView ni
JavaScript. Pero el pipeline de release sí depende de Node.js: `.github/workflows/release.yml`
declara `NODE_VERSION: "22"`, usa `actions/setup-node@v4` en dos jobs y ejecuta tres
scripts `.mjs`; hay 8 archivos `.mjs` versionados (7 en `scripts/`, más `tests/*.test.mjs`).
Detalle en `docs/product-audit.md` §8 y §9. `ci.yml` sí está libre de Node.

Consecuencia práctica del principio: **la ausencia de Node en el artefacto no autoriza a
afirmar que el proyecto no requiere Node** (Req 14.5). El comentario de cabecera de
`.github/workflows/ci.yml` que hoy afirma "la aplicación es 100% Rust sobre iced" viola el
Req 14.5 y lo reescribe la tarea 13.1.

**Frontera declarada (Python).** Los scripts de medición de `docs/baseline-evidence/`
(`*.py`) son instrumentos de auditoría de solo lectura: no participan del build, del
empaquetado, del release ni de la ejecución del producto. El alcance de P1 es el workspace
de Cargo y la cadena de build/CI, no las herramientas de medición de la auditoría. Esta
frontera se declara acá para que no se lea como excepción silenciosa; migrar esas
mediciones al futuro crate `xtask` es Hito 3 (`docs/product-audit.md` §8).

### P2 — El núcleo no conoce la UI

**Enunciado.** `midway-core` se mantiene libre de dependencias de iced, de widgets, de
colores y de layout. El dominio y la infraestructura no importan tipos de presentación; la
dirección de dependencia es siempre `midway-desktop` → `midway-core`.

**Requisito de origen.** Req 11.2.

**Verificación.** Cuatro evidencias reproducibles, documentadas con sus comandos en
`docs/architecture.md` §1.4: el manifiesto (`midway-core/Cargo.toml`), el lockfile (nodo
`midway-core` de `Cargo.lock`), el subgrafo transitivo
(`python3 docs/baseline-evidence/subgraph_probe.py`) y el fuente
(`grep -rn 'iced' midway-core --include='*.rs' --include='*.toml'`).

**Estado observado.** Cumple, **sin barrera automatizada**. El subgrafo de `midway-core`
tiene 229 paquetes y cero de la familia iced; el de `midway-desktop` tiene 493, trece de
ellos iced. El único `grep` que devuelve `iced` en `midway-core` es un comentario de
trazabilidad de un test. El test que existe verifica Tauri, no iced: su filtro es
`name == "tauri" || name.starts_with("tauri-")`. Convertirlo en tripwire de iced es un
cambio de código y quedó fuera del alcance de la tarea 2.4
(`docs/architecture.md` §1.4).

### P3 — Lo específico de plataforma vive en un solo lugar

**Enunciado.** Cuando una entrega introduzca una integración específica de sistema
operativo, esa integración se encapsula en un módulo `platform`. No se dispersan bloques
`#[cfg(target_os = "...")]` por los módulos de UI ni de dominio.

**Requisito de origen.** Req 11.3. Relacionado: Req 11.4, que registra Linux Wayland,
Linux X11, Windows y macOS como plataformas de primer nivel junto con su estado de
verificación real (`docs/product-audit.md` §14).

**Verificación.** `grep -rn "target_os" midway-core/src midway-desktop/src --include='*.rs'`.

**Estado observado.** No aplica todavía, y por eso es fácil de cumplir: el comando devuelve
**0** ocurrencias y no existe módulo `platform`. El principio es preventivo: el primer
`#[cfg(target_os)]` que entre debe entrar dentro de un módulo `platform`, no al lado del
código que lo necesita.

### P4 — La versión de iced no se mueve por conveniencia

**Enunciado.** iced queda fijo en la versión pinneada del `Cargo.lock` (`=0.14.0`).
Cualquier actualización exige ADR propio, verificación multiplataforma y plan de rollback, y
está fuera del alcance de este spec.

**Requisito de origen.** Req 11.10. Registrado como decisión en
`docs/adr/0002-iced-pinneado.md`.

**Verificación.** `grep -n 'iced = ' midway-desktop/Cargo.toml` (pin exacto `=0.14.0`) y el
nodo `iced` de `Cargo.lock`. Ambos datos están capturados en
`docs/baseline-evidence/INDEX.md` §"Entorno de ejecución".

**Estado observado.** Cumple. `Cargo.toml` declara `iced = { version = "=0.14.0", ... }` y
`Cargo.lock` resuelve `iced 0.14.0` (los subcrates en `0.14.0`, salvo `iced_widget` en
`0.14.2`).

## 3. Principios de datos y de secretos

### P5 — Un secreto nunca sale de su lugar

**Enunciado.** Los valores de secretos se mantienen fuera de los logs, las exportaciones,
el historial, los diagnósticos, los tests y los mensajes de error. Lo que puede viajar es
el **alias**, nunca el valor. Todo camino que renderice un request para mostrarlo,
guardarlo o exportarlo usa redacción, no resolución.

**Requisito de origen.** Req 11.6.

**Verificación.** Revisión del fuente en los puntos de salida, más lectura del modo de
render usado en cada camino:

```bash
grep -rn "SecretRenderMode" midway-core/src midway-desktop/src --include='*.rs'
```

`midway-core/src/domain/interpolation.rs` define `SecretRenderMode::{Resolve, Redact}` y
`REDACTION_TOKEN`. La estructura exportable `SecretMetadata`
(`midway-core/src/domain/workspace.rs:100`) contiene `alias`, `created_at` y `updated_at`:
**por diseño no tiene campo de valor**, y `make_native_bundle` limpia `snapshot.secrets`
cuando el export no incluye metadata de secretos
(`midway-core/src/domain/interop.rs:136`).

**Estado observado.** Brecha registrada: `L27`, Hito 10. El envío resuelve el request con
`SecretRenderMode::Resolve` y persiste la URL resuelta en `history.url`; si un
`{{secret:...}}` se interpola en la URL o en el query string, su valor queda en claro en
`workspace.sqlite3`, se muestra en la sección History y viaja en el export nativo cuando
incluye historial. El preview sí usa `Redact`
(`midway-desktop/src/app.rs`, `execute_send` vs `compute_preview`).

Regla operativa que se deriva: **ninguna entrega de este spec agrega un camino nuevo que
escriba un request resuelto en almacenamiento persistente, en un archivo de exportación o
en un mensaje de error.** La brecha existente no autoriza a ampliarla.

### P6 — La persistencia no se toca a la ligera

**Enunciado.** Todo cambio de persistencia exige versión de esquema, respaldo previo,
migración transaccional, validación posterior, rollback y tests desde versiones anteriores.
Esto rige **con independencia** de que el cambio sea compatible hacia atrás y del tamaño del
archivo modificado. Además, ninguna entrega destruye datos existentes del usuario.

**Requisito de origen.** Req 11.5, más el Req 13.11 (preservar los datos existentes del
usuario) y el Req 13.7 (el estado del usuario se persiste y se recupera tras reiniciar).

**Verificación.** El tripwire `SESSION_SCHEMA_VERSION == 1` de
`midway-desktop/src/session.rs` (tarea 4.3) falla si alguien cambia el esquema de sesión sin
declararlo. Para SQLite no hay tripwire equivalente.

**Estado observado.** Brechas registradas: `L25` (la persistencia SQLite no tiene versión de
esquema: `migrate()` es `CREATE TABLE IF NOT EXISTS` más un `ALTER TABLE` condicional por
`PRAGMA table_info`, sin `user_version`, sin respaldo, sin migración transaccional, sin
validación posterior y sin rollback) y `L26` (la sesión no migra), ambas Hito 6.

Consecuencia para este spec: **el esquema no cambia.** `SESSION_SCHEMA_VERSION` queda en 1 y
la altura del panel de respuesta viaja por el campo `panelSizes.responsePanelHeight` que ya
existe. Si alguna tarea descubre que necesita cambiar el esquema, se detiene y vuelve a
diseño bajo el Req 11.5 completo.

## 4. Principios de código

### P7 — Un pánico posible es una decisión, no un descuido

**Enunciado.** El código de producción tocado por una entrega evita `unwrap()` y `expect()`
**sin justificación escrita adyacente**. La justificación explica por qué el caso de error
es imposible o por qué abortar es la respuesta correcta; "acá no puede fallar" sin argumento
no es justificación.

**Requisito de origen.** Req 11.7. Relacionado: Req 13.4 (manejo de errores en los caminos
que la entrega introduce).

**Verificación.**

```bash
python3 docs/baseline-evidence/count_unwrap_prod.py
```

El script excluye los bloques `#[cfg(test)]` balanceando llaves, así que su recuento es de
código de producción. El conteo por archivo de `L21` (358 ocurrencias) **incluye** los
módulos de test alojados en esos mismos archivos y no debe leerse como recuento de
producción.

**Estado observado.** Ocho ocurrencias fuera de `#[cfg(test)]` en el baseline, con
justificación desigual:

| Ubicación | Ocurrencias | ¿Justificación escrita? |
| --- | --- | --- |
| `midway-desktop/src/ui/activity_bar.rs:71,81,82,86,87` (`collection_initials`) | 5 | Parcial: el doc-comment de la función enumera los cuatro casos y el código guarda `trimmed.is_empty()` antes del primer `unwrap()`, pero ninguna línea de `unwrap()` lleva su propia justificación |
| `midway-desktop/src/app.rs:1239` (`ProgressReceiverHandle::take`) | 1 | Sí: el mensaje del `expect` dice por qué (mutex que no debería envenenarse) |
| `midway-desktop/src/main.rs:70` (`boot`) | 1 | Sí para el `block_on`, no para el `expect`: el doc-comment justifica resolver `AppState::initialize` de forma bloqueante, y no justifica abortar el arranque si falla. Registrado como `L20` |
| `midway-desktop/src/app.rs:1716` | 1 | No es código: es un doc-comment que menciona `.unwrap()` al describir el manejo de payloads de pánico |

Descontada esa última fila, el recuento efectivo de **llamadas** de producción es 7.

Brecha registrada: `L21` (Hito 2) para el volumen total, `L20` (Hito 4) para el pánico de
arranque. Regla operativa: **las entregas de este spec no agregan ocurrencias nuevas sin
justificación adyacente**, y `activity_bar.rs` no se reescribe acá (no está en el alcance de
ninguna tarea).

### P8 — La UI no se congela

**Enunciado.** El código de producción tocado por una entrega evita bloquear el hilo de UI:
el trabajo de I/O y de cómputo viaja por `Task`/`subscription`, los canales son acotados, la
cancelación se propaga y no quedan tareas huérfanas.

**Requisito de origen.** Req 11.8. Relacionado: Req 13.8 (toda operación de larga duración
es cancelable).

**Verificación.**

```bash
grep -rn "block_on\|unbounded\|thread::sleep" midway-core/src midway-desktop/src --include='*.rs'
```

Cada resultado se clasifica a mano en producción o test: los `block_on` de los módulos de
test son el patrón normal para ejercitar código `async` desde un test síncrono y no son
violaciones.

**Estado observado.** Cumple parcialmente, con tres hechos que conviene tener escritos:

1. **Un `block_on` de producción, justificado.** `midway-desktop/src/main.rs:69`, en `boot`.
   iced 0.14 espera que `boot` devuelva el estado inicial de forma síncrona y
   `AppState::initialize` es `async`; el doc-comment de la función explica la decisión. Es
   arranque, no interacción: no congela una UI ya dibujada.
2. **El autosave de sesión no escribe en el hilo de UI.** `update_session` arma el snapshot
   y delega la escritura a `Task::perform`
   (`midway-desktop/src/app.rs`, `SessionMessage::AutosaveTick` →
   `crate::session::write_session_snapshot`).
3. **Un canal no acotado en producción.** `midway-desktop/src/app.rs:1847`
   (`RunnerMessage::StartRequested`) crea el canal de progreso del Collection_Runner con
   `mpsc::unbounded()`. El Req 11.8 pide canales acotados: es una brecha del baseline. No la
   cierra ninguna tarea de este spec, y el Collection_Runner tampoco es disparable desde la
   UI hoy (`L12`, Hito 4), así que el canal no tiene productor real en producción. La
   cancelación existe como `oneshot` en el runner, y `RequestExecutorHandle::cancel` existe
   sin llamador en `midway-desktop` (`L17`, Hito 4).

Regla operativa: **ningún camino nuevo introducido por este spec hace I/O sincrónico dentro
de `update` ni de `view`, y ningún canal nuevo se crea sin cota.**

### P9 — Un archivo grande necesita defensa escrita

**Enunciado.** Si un archivo tocado por una entrega supera las 1 000 líneas, la entrega
incluye un ADR que lo justifique. Un archivo nuevo se mantiene por debajo de ese límite.

**Requisito de origen.** Req 11.9. Relacionado: Req 8.8 (el módulo de la vertical queda
por debajo de 1 000 líneas) y Req 8.9 (se registra la reducción de líneas de `app.rs`).

**Verificación.**

```bash
find midway-core/src midway-desktop/src -name '*.rs' -exec wc -l {} + | sort -rn | head
```

**Estado observado.** Brechas registradas: `L8` (`midway-desktop/src/app.rs`, 10 750 líneas)
y `L9` (seis archivos más sobre 1 000: `sqlite_repository.rs` 2 322, `updater.rs` 2 311,
`request_tree_pane.rs` 2 115, `interop.rs` 2 068, `collection_runner.rs` 1 497,
`session.rs` 1 054), Hito 2 con reducción parcial en el Hito 1. El plan de descomposición
por grupos de mensajes está en `docs/adr/0004-descomposicion-de-app-rs.md`; la justificación
del tamaño de cada archivo que **permanezca** sobre 1 000 líneas al cierre corresponde al
ADR 0006, que todavía no existe en `docs/adr/` y se escribe al cierre del spec (Req 7.6).

## 5. Principios de entrega

### P10 — Se entrega software, no documentación de software

**Enunciado.** Toda entrega incluye código funcionando, además de la documentación de
auditoría que los Requisitos 3, 4 y 5 exigen. La documentación es condición, no sustituto.

**Requisito de origen.** Req 14.1. Relacionado: Req 13.1 a 13.9 (definición de terminado).

**Verificación.** Revisión humana del plan de tareas: el spec debe contener tareas que
modifiquen `.rs` de producción y no solo `.md`.

**Estado observado.** Cumple por construcción del plan: las tareas 1 a 4 no tocan código de
producción y el primer cambio en un `.rs` de producción llega en la tarea 6 (creación de la
vertical `ui/theme_settings.rs`), seguido de la tarea 7 (cableado), la tarea 10 (divisor
horizontal del panel de respuesta) y la tarea 11 (estados vacíos y jerarquía visual). La
secuencia "evidencia antes que código" (Req 1.5) **no** convierte al spec en un spec de
documentación.

### P11 — Nada que se vea sin que funcione

**Enunciado.** La entrega evita pestañas vacías, botones permanentemente deshabilitados y
funcionalidades simuladas. Un estado vacío nombra la acción siguiente disponible; si la
acción no aplica en ese contexto, se renderiza **sin botón** en vez de con un botón muerto.
Durante periodos breves de carga se muestra el mismo componente con progreso, no un área en
blanco.

**Requisito de origen.** Req 14.2, más el Req 9.5 (estado vacío accionable, incluso durante
carga) y el Req 13.6 (todo comportamiento visible es alcanzable desde la UI).

**Verificación.** Revisión humana de cada superficie tocada, más los tests de ejemplo de las
especificaciones de `EmptyState` (tarea 11.4), que fijan título, hint y presencia o ausencia
de acción por estado vacío.

**Estado observado.** Brechas registradas, varias de ellas cerradas por este spec:

- `L30` (Hito 1, tarea 11.2): cuatro estados vacíos informan la ausencia sin ofrecer la
  acción siguiente — "Sin respuesta aún", "No hay cookies almacenadas",
  "Sin assertions configuradas" y "Midway Desktop — no hay tabs abiertas".
- `L12`, `L13`, `L14`, `L16`, `L17` (Hito 4): funcionalidad implementada e inalcanzable
  desde la UI. El caso más claro es la vista del modo Test, que dice "Presioná Run" sin que
  ese control exista: eso es exactamente lo que P11 prohíbe.
- `L10`, `L11` (Hito 4): variantes de mensaje cuyo handler devuelve `Task::none()`, marcadas
  `#[allow(dead_code)]`. En `docs/feature-matrix.md` figuran como `Scaffolded`, no como
  funcionalidad.

**Deshabilitado por contexto ≠ deshabilitado permanentemente.** La pestaña Test se
deshabilita cuando no hay colección activa
(`midway-desktop/src/ui/top_bar.rs:256`, `is_disabled`), y eso **no** viola P11: la
condición es real, temporal y depende de una acción del usuario que sí existe. Lo prohibido
es el control que nunca se habilita.

### P12 — No se reencapsula el monolito

**Enunciado.** La entrega evita introducir un archivo nuevo que solo reencapsule el mismo
monolito. Extraer significa mover comportamiento y **borrar el original**, no crear una capa
de reenvío que deje las dos versiones vivas.

**Requisito de origen.** Req 14.3. Relacionado: Req 8.6 (la vertical no conoce el `Message`
raíz ni el estado global) y Req 8.1.

**Verificación.** Revisión humana del diff, más el test de guardia de importaciones
`midway-desktop/tests/vertical_import_guard.rs` (tarea 6.3), que falla si
`src/ui/theme_settings.rs` referencia `AppState`, `crate::session`, `reqwest` o
`midway_core::infra`.

**Estado observado.** Cumple por construcción del plan: la tarea 7.2 elimina `fn update_theme`
y la definición de `enum ThemeMessage` de `app.rs` en el mismo cambio en que `top_bar.rs`
empieza a consumir `theme_settings::control`. El `pub use` que se conserva es un reexport de
nombre para no romper sitios de construcción existentes, no una capa de reenvío de
comportamiento. La medición de la reducción de líneas de `app.rs` (tarea 8.2) es la
evidencia de que la extracción movió peso y no lo duplicó.

### P13 — Cambios revisables, uno por vez

**Enunciado.** El trabajo se divide en cambios revisables por tarea. Cada tarea deja el
workspace compilable y es revisable de forma independiente.

**Requisito de origen.** Req 14.4.

**Verificación.** `cargo check --workspace --all-targets --all-features` al cierre de cada
tarea de código. En este entorno, `cargo fmt` y `cargo clippy` están `Sin_Herramienta`
(`L4`), así que el gate local es más angosto que el declarado: eso se registra, no se
disimula.

**Estado observado.** Cumple por construcción del plan. Brechas de gate registradas: `L2`
(el target `verify` del `Makefile` corre solo `check test`; lo extiende la tarea 13.2) y
`L3` (CI no verifica formato ni lints; lo corrige la tarea 13.1).

### P14 — Ninguna afirmación sin evidencia reproducible

**Enunciado.** No se afirma "paridad total de funcionalidades", "100% compatible", "listo
para producción", "verificado multiplataforma", "cero regresiones", "enteramente Rust", "no
requiere Node" ni "migración completa" sin evidencia reproducible registrada. Si no existe
esa evidencia, la afirmación se registra **como no verificada**, en vez de omitirse o
suavizarse.

**Requisito de origen.** Req 14.5 y 14.6. Relacionado: Req 4.4 (ninguna fila en `Verified`
con solo un test unitario como evidencia), Req 4.5 y 4.8
(`No_Verificable_En_Entorno` donde la verificación gráfica no se realizó) y Req 1.4
(`Sin_Herramienta` no se declara como verificación que pasó).

**Verificación.** Revisión humana de todo texto entregado, más
`python3 docs/baseline-evidence/verify_feature_matrix.py`, que comprueba la forma de la
matriz: estados de la lista cerrada, ninguna fila en `Verified`, ningún porcentaje y
`No_Verificable_En_Entorno` en toda la columna de verificación manual.

**Estado observado.** Dos violaciones observadas en el repositorio, ambas con dueño:

- El comentario de cabecera de `.github/workflows/ci.yml` afirma "la aplicación es 100% Rust
  sobre iced" sin evidencia registrada, y el `release.yml` del mismo directorio depende de
  Node.js. Lo reescribe la tarea 13.1.
- El `README.md` describe un producto más terminado que el que existe: las discrepancias
  fila por fila están en `docs/product-audit.md` §10.

Límites de verificación de este entorno, que ningún documento debe pisar: no se abrió
ninguna ventana (`DISPLAY` no definida, solo `WAYLAND_DISPLAY=wayland-1`), así que toda
funcionalidad con superficie gráfica queda `No_Verificable_En_Entorno` (`L34`); y `cargo fmt`,
`cargo clippy`, `cargo deny`, `cargo audit` y `cargo llvm-cov` no están instalados, así que
el estado de formato, lints, licencias, vulnerabilidades y cobertura es **desconocido**, no
"bueno" (`L4`).

## 6. Principio de identidad visual

### P15 — Identidad propia, no la de la competencia

**Enunciado.** Midway **no reproduce** la identidad visual, los logotipos, las paletas ni la
presentación comercial de Postman, Insomnia, Bruno, Linear, Raycast, Arc ni JetBrains. No se
copian marcas, no se importan paletas de esos productos, no se imita su lenguaje de
marketing y no se usan sus nombres para describir la apariencia de Midway.

**Requisito de origen.** Req 9.8.

**Qué prohíbe y qué no.** La distinción importa porque el producto es un cliente de API y la
interoperabilidad con formatos ajenos es parte de su función:

| Permitido | Prohibido |
| --- | --- |
| Soportar el **formato de datos** Postman Collection v2.1 en import y export, y nombrarlo como formato en la UI (`"Postman Collection v2.1"` en `ui/workspace_panel.rs`) | Presentar Midway como Postman, usar su logotipo, su tipografía de marca o su paleta |
| Resolver problemas de UX que esos productos también resolvieron (paneles redimensionables, paleta de comandos, tema claro/oscuro): son patrones de interacción, no identidad | Copiar sus valores de color, su iconografía o su composición de pantalla como referencia visual directa |
| Usar la escala de elevación propia de la paleta de `DesignSystem` (`background_primary`, `background_secondary`, `surface_elevated`, `border`, en `ui/design_system.rs:83-88`) para dar jerarquía | Introducir colores de marca nuevos que provengan de esos productos |

**Verificación.** Revisión humana del diff visual, más:

```bash
grep -rn "Postman\|Insomnia\|Bruno\|Raycast\|JetBrains" midway-core/src midway-desktop/src --include='*.rs'
```

El comando omite deliberadamente "Arc" y "Linear": `std::sync::Arc` aparece en todo el
fuente y `Linear` aparece dentro de `linearize` en los cálculos de luminancia de
`ui/design_system.rs`, así que un grep de esos dos términos devuelve casi solo falsos
positivos. Para esos dos productos la verificación es revisión humana del texto visible y de
la paleta.

Cada resultado se clasifica en tres categorías: **formato de interoperabilidad** (permitido),
**comentario del fuente** (deuda de proceso) o **cadena visible al usuario que presenta a
Midway como otro producto** (violación).

**Estado observado.** Ninguna violación de identidad visual, y deuda de proceso registrada.
Ninguna cadena visible al usuario contiene "Insomnia", "Bruno", "Raycast" ni "JetBrains"; las
apariciones de "Postman" en texto visible son etiquetas del formato de import/export. Lo que
sí queda: seis comentarios del fuente describen la apariencia por referencia a un producto
ajeno —`ui/design_system.rs:67` ("Tema oscuro, usado por defecto (estilo Insomnia)"),
`ui/design_system.rs:204` ("Insomnia-style"), `ui/tab_bar.rs:2` ("Matches the Insomnia-style")
y tres menciones a "rediseño Insomnia" en `app.rs:705`, `app.rs:1390` y `app.rs:5543`—.

Eso es **deuda de proceso**, parte de `L33` (Hito 3): el código no copia una paleta ajena,
pero describe la propia como derivada de otra marca, y ese vocabulario es el que después
termina en un README o en una captura. Regla operativa: **ninguna entrega de este spec agrega
comentarios ni documentación que describan la apariencia de Midway por referencia a un
producto ajeno.** La tarea 11.3 aplica jerarquía visual sin colores de marca nuevos y sin
reproducir la identidad de ninguno de los siete productos nombrados.

## 7. Tabla resumen

| ID | Principio | Requisito | Estado observado |
| --- | --- | --- | --- |
| P1 | Un solo stack: Rust nativo, sin runtimes web ni ajenos | 11.1, 13.9, 10.8 | Cumple en el artefacto; brecha en el proyecto (Node en `release.yml`, §8-§9 de `product-audit.md`) |
| P2 | El núcleo no conoce la UI | 11.2 | Cumple, sin barrera automatizada de iced |
| P3 | Lo específico de plataforma vive en un solo lugar | 11.3, 11.4 | No aplica todavía: cero `#[cfg(target_os)]`, sin módulo `platform` |
| P4 | La versión de iced no se mueve por conveniencia | 11.10 | Cumple: pin `=0.14.0`, ADR 0002 |
| P5 | Un secreto nunca sale de su lugar | 11.6 | Brecha `L27` (historial), Hito 10 |
| P6 | La persistencia no se toca a la ligera | 11.5, 13.7, 13.11 | Brechas `L25`, `L26`, Hito 6; este spec no cambia el esquema |
| P7 | Un pánico posible es una decisión, no un descuido | 11.7, 13.4 | 8 ocurrencias de producción, justificación desigual; `L21` Hito 2, `L20` Hito 4 |
| P8 | La UI no se congela | 11.8, 13.8 | Cumple parcialmente: `block_on` de arranque justificado; canal `unbounded` del runner sin cota |
| P9 | Un archivo grande necesita defensa escrita | 11.9, 8.8, 8.9 | Brechas `L8`, `L9`, Hito 2; ADR 0004 y ADR 0006 |
| P10 | Se entrega software, no documentación de software | 14.1, 13.1-13.9 | Cumple por construcción del plan (tareas 6, 7, 10, 11) |
| P11 | Nada que se vea sin que funcione | 14.2, 9.5, 13.6 | Brechas `L30` (Hito 1, tarea 11.2), `L10`-`L17` (Hito 4) |
| P12 | No se reencapsula el monolito | 14.3, 8.6 | Cumple por construcción; guardia en tarea 6.3 |
| P13 | Cambios revisables, uno por vez | 14.4 | Cumple; brechas de gate `L2`, `L3` (tareas 13.1, 13.2) |
| P14 | Ninguna afirmación sin evidencia reproducible | 14.5, 14.6, 4.4, 4.8, 1.4 | Violaciones observadas: comentario de `ci.yml` (tarea 13.1) y README (`product-audit.md` §10) |
| P15 | Identidad propia, no la de la competencia | 9.8 | Sin violación visual; deuda de proceso en 6 comentarios (`L33`, Hito 3) |

## 8. Uso en revisión

Estos quince principios son el criterio de rechazo de un cambio. En una revisión se aplican
así:

1. **P1, P2, P4** se comprueban con los comandos de su sección antes de mirar el diff: son
   preguntas sobre el grafo de dependencias, no sobre el código.
2. **P5, P7, P8** se comprueban sobre las líneas agregadas, no sobre el archivo entero. La
   pregunta no es "¿este archivo cumple?" sino "¿este diff empeora el estado registrado?".
3. **P9, P12, P13** se comprueban sobre la forma del cambio: tamaño, dirección de la
   extracción, y si el workspace compila al final de la tarea.
4. **P11 y P15** requieren mirar la superficie visible, y en este entorno eso no se puede
   hacer en pantalla (`L34`): se revisa el código de la vista y se registra la verificación
   gráfica como `No_Verificable_En_Entorno`.
5. **P14** se aplica al texto del propio cambio, incluidos mensajes de commit, comentarios y
   documentación. Es el principio que más fácil se rompe al escribir, no al programar.

Cuando un principio y una tarea entran en conflicto, gana el principio y se detiene la
tarea: el spec ya declara ese comportamiento para el caso del Req 11.5 (cambio de esquema de
persistencia), y la misma regla vale para el resto.
