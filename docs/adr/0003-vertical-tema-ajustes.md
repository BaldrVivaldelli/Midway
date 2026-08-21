# ADR 0003 — La primera vertical extraída de `app.rs` es Tema/Ajustes

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 7.3 (con efecto sobre 8.1, 8.2, 8.4)
- **Evidencia**: fuente en el HEAD del ADR 0001
  (`1fc270326e5c304f24ce08fe1f528da33a007d3e`); líneas atribuidas con
  `docs/baseline-evidence/attribute_app_rs.py` y `fn_inventory.py`; las citas
  `archivo:línea` de este ADR las verifica
  `docs/baseline-evidence/verify_adr_citations.py`

## Contexto

`midway-desktop/src/app.rs` tiene 10 750 líneas y concentra el `Message` raíz, el `update`
completo y el estado agregado de la aplicación. El Hito 1 exige extraer **una** vertical
autocontenida (estado propio, mensajes propios, `update`, `view` y tests propios) que
comunique hacia arriba mediante un Evento_Ascendente. La elección de *cuál* vertical
determina qué se aprende del primer intento: si la vertical elegida arrastra
infraestructura, el riesgo del patrón de extracción se mezcla con el riesgo del dominio y
un fallo no dice cuál de los dos falló.

El `Message` raíz agrupa por área en quince variantes (verificado leyendo
`midway-desktop/src/app.rs:58-88`). Medición de las candidatas de menor superficie, con
líneas de producción del handler correspondiente y sus auxiliares atribuibles, obtenidas
con `docs/baseline-evidence/attribute_app_rs.py`:

| Grupo candidato | Variantes | Handler | Líneas de producción | Toca infraestructura |
| --- | --- | --- | --- | --- |
| `Theme` | 1 (`Toggled`) | `update_theme` (`app.rs:2123-2131`) | 9 | No |
| `ResponseInspector` | 1 (cambio de tab) | `update_response_inspector` (`app.rs:4319-4330`) | 12 | No, pero muta la tab activa |
| `Updater` | 1 (relleno) | ninguno: `Message::Updater(_) => Task::none()` (`app.rs:1815`) | 1 | Scaffolding, `#[allow(dead_code)]` |
| `TopBar` | 3 | `update_top_bar` + `enforce_top_bar_mode_after_collection_change` | 41 | No, pero depende de `workspace.collections` |
| `Palette` | 4 | `update_palette` + `build_palette_items` + `execute_palette_item` + `open_saved_request_in_tab` | 116 | Lee `workspace.collections` y abre tabs |
| `PanelResize` | 6 | `update_panel_resize` + `panel_resize_message_for_event` + `vertical_resize_divider` + tres funciones puras | 122 | No |

La superficie completa de Tema/Ajustes en el baseline, verificada leyendo el fuente:

- `enum ThemeMessage { Toggled }` — `app.rs:565-569`.
- `fn update_theme` — `app.rs:2123-2131`. Su cuerpo entero son dos asignaciones:
  `state.theme_mode = state.theme_mode.toggled();` y `state.session.dirty = true;`, y
  devuelve `Task::none()`.
- Campo `Midway.theme_mode: ThemeMode` — `app.rs:1380`.
- Control de tema del Top_Bar — `ui/top_bar.rs:144-160`: un `match` de ícono
  (`☀` / `☾`) y un `button` con `on_press(Message::Theme(ThemeMessage::Toggled))`.
- Derivación de paleta `DesignSystem::for_mode` — `ui/design_system.rs:135`.
- Puntos de lectura del modo: `main.rs:32` y `main.rs:44` (función `theme` de
  `iced::application`), `app::view` (`app.rs:5560`), `ui/top_bar.rs:144`,
  `ui/response_inspector.rs:48` y `:61`, más los constructores de test.
- Persistencia: `SessionSnapshot.theme_mode` (`session.rs:124`), escrito en
  `build_session_snapshot` (`app.rs:2899`) y restaurado en `restore_pending_session`
  (`app.rs:1539`) y `Midway::new` (`app.rs:1442`).

Dos hallazgos del fuente hacen a esta vertical interesante en vez de trivial:

1. **`update_theme` escribe `state.session.dirty = true` con la mano.** La lógica de tema
   está acoplada al Session_Store aunque no tenga nada que ver con él. Ese acoplamiento es
   exactamente lo que el contrato de Evento_Ascendente tiene que romper.
2. **El modo de tema está almacenado dos veces.** Existe `Midway.theme_mode`
   (`app.rs:1380`) y también `Midway.session.theme_mode` dentro de `SessionStoreState`
   (`app.rs:1302`). `restore_pending_session` escribe en `session.theme_mode`
   (`app.rs:1539`), `Midway::new` lo copia a `theme_mode` (`app.rs:1442`) y
   `build_session_snapshot` lee `state.theme_mode` (`app.rs:2899`). Después del arranque,
   `session.theme_mode` queda desactualizado en cada toggle y nadie lo vuelve a leer: el
   toggle solo escribe el campo de nivel superior. Es una duplicación con una copia muerta.

## Decisión

1. La única vertical extraída en este spec es **Tema/Ajustes**, hacia
   `midway-desktop/src/ui/theme_settings.rs`.
2. **Superficie mínima y completa.** Es cerrada y abarca todo lo que hace falta para que la
   funcionalidad viva entera dentro del módulo: `ThemeMessage`, la lógica de `update_theme`, el
   campo de modo de tema (que pasa de `Midway.theme_mode: ThemeMode` a
   `Midway.theme: ThemeSettingsState`), el control de tema del Top_Bar y la derivación de
   `DesignSystem::for_mode`.
3. La vertical **no** ejecuta el efecto de persistencia. `update` devuelve
   `vec![ThemeEvent::ThemeChanged { mode }]` y la App_Raíz traduce ese evento a
   `state.session.dirty = true`, que dispara el autosave ya existente.
4. `control` devuelve `Element<'a, ThemeMessage>`, no `Element<'a, Message>`: el módulo no
   conoce el `Message` raíz. `top_bar.rs` hace el `.map(Message::Theme)`.
5. `control` recibe `ThemeSettingsState` por valor y `&DesignSystem` por parámetro. No
   recibe `&Midway`. Sin lectura de estado global.
6. **Cero persistencia nueva.** `SESSION_SCHEMA_VERSION` queda en 1, el campo persistido
   sigue llamándose `theme_mode` y su nombre JSON no cambia. La vertical no toca el
   esquema.
7. **La copia muerta `SessionStoreState.theme_mode` no se elimina en este spec.** Es el
   canal por el que `restore_pending_session` entrega el modo restaurado (`app.rs:1539` →
   `app.rs:1442`), y borrarla obligaría a rediseñar cómo se pasa ese valor al construir
   `Midway`. Queda registrada como Deuda_Registrada **por este ADR**, con hito propietario
   Hito 2, dentro de la vertical de sesión (grupo `Session` del ADR 0004). Precisión, para
   no afirmar lo que no está hecho: al momento de escribir este ADR
   `docs/known-limitations.md` **no** tiene fila para esta duplicación y ninguna tarea de
   este spec la agrega, así que el registro vigente es este documento. No se resuelve acá.
8. **Cero infraestructura.** El módulo no importa `AppState`, `crate::session`, `reqwest`
   ni `midway_core::infra`. Un test de guardia de importaciones
   (`midway-desktop/tests/vertical_import_guard.rs`) fija esa restricción como test, no
   como convención.
9. El comportamiento observable no cambia. El test de caracterización del toggle de tema
   se escribe **antes** de la extracción, sobre el baseline sin modificar, y se adapta a
   la ruta nueva sin cambiar ningún valor esperado.
10. No se crea un trait genérico de vertical ni una capa intermedia de "verticales". Con
    una sola implementación, un trait es especulación. El patrón se registra en
    `docs/architecture.md` como prosa reproducible, no como abstracción de código.

### Alternativas descartadas

- **`Updater`**: es la superficie más chica (una línea de routing a `Task::none()`), pero
  es scaffolding con `#[allow(dead_code)]`. Extraer código muerto no ejercita ningún
  contrato y no demuestra invariancia de comportamiento, porque no hay comportamiento.
- **`ResponseInspector`**: 12 líneas, pero su handler necesita `state.active_tab` y
  `state.tabs` para mutar la tab activa. Su estado no es propio: vive dentro de
  `RequestTabState`. Extraerlo obligaría a decidir la propiedad del estado de tabs, que es
  un problema mucho mayor que la vertical.
- **`PanelResize`**: 122 líneas, mayormente aritmética pura, buena candidata en abstracto,
  pero este mismo spec la modifica en el incremento de UX (divisor horizontal del panel de
  respuesta, cambio de forma de `DividerDragged`). Extraer y modificar a la vez mezcla dos
  clases de riesgo y arruina la lectura del diff.
- **`Palette`**: 116 líneas y depende de `workspace.collections` para construir sus ítems
  y de la apertura de tabs para ejecutarlos. Es candidata siguiente, no la primera.
- **`RequestComposer`**: 1 174 líneas de handlers, ejecución HTTP real, persistencia de
  requests y prompts de cambios sin guardar. Es el objetivo final, no el primer paso.

## Consecuencias

- El primer ejercicio del patrón aísla **una** variable: la estructura. Si algo falla, es
  el patrón, no el dominio ni la infraestructura.
- El contrato de Evento_Ascendente queda ejercitado de verdad. Tema/Ajustes tiene un
  efecto fuera de su alcance (marcar la sesión sucia), así que el paso 6 del patrón
  ("traducir eventos en la raíz") se implementa y se testea, en vez de quedar como
  intención escrita.
- La reducción de líneas de `app.rs` es **pequeña**: 9 líneas de handler más la definición
  del enum, es decir, orden de decenas, no de miles. Eso se acepta y se registra
  explícitamente en `docs/product-audit.md` con el recuento antes y después. El valor de
  esta tarea es el patrón, no el conteo. Cualquier lectura del resultado como "el monolito
  ya está resuelto" es incorrecta.
- El costo de equivocarse es bajo y el rollback es local: revertir la vertical es borrar un
  módulo y restaurar un campo.
- Aparece un beneficio colateral verificable: `main.rs`, `app::view`,
  `ui/response_inspector.rs` y `ui/top_bar.rs` dejan de leer un campo suelto y pasan a
  leer `state.theme.mode()` / `state.theme.design_system()`. La derivación de paleta queda
  en un solo lugar.
- La duplicación `Midway.theme_mode` / `SessionStoreState.theme_mode` sobrevive a la
  extracción, con una diferencia: queda **escrita** en el punto 7 de este ADR. Hoy es un
  accidente silencioso; al cierre de este spec es deuda con dueño (Hito 2).
- El módulo nuevo queda muy por debajo del límite de 1 000 líneas, sin imponer un mínimo:
  una vertical chica no es un defecto.
- La verificación gráfica del control de tema **no** se puede hacer en este entorno
  (`DISPLAY` vacío, solo `WAYLAND_DISPLAY=wayland-1`). Se registra como
  `No_Verificable_En_Entorno` en `docs/feature-matrix.md`. La evidencia disponible es de
  transición de estado y de eventos emitidos, ejecutable sin ventana.
- Las cuatro verticales siguientes quedan en orden de prioridad (selección de environment,
  paleta de comandos, historial, ciclo de vida de ejecución de request) y su plan general
  vive en el ADR 0004. Este ADR no las diseña.
- Elegir otra vertical primera, o extraer varias en una sola pasada, requiere un ADR nuevo
  que supersede a este.
