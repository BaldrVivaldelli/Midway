# Reporte de diferencias clasificadas — Midway_Tauri vs. Midway_Desktop

> **Reporte de diferencias encontradas (Tarea 17.3, Requisito 10, Criterio 10.7).**
> Este documento cierra el ciclo de comparación funcional: toma cada diferencia
> registrada durante la comparación manual (Tarea 17.2 →
> [`docs/functional-comparison.md`](./functional-comparison.md), en particular la
> *Parte E.4*) y el resumen escrito de equivalencia (Tarea 17.1 →
> [`docs/functional-equivalence.md`](./functional-equivalence.md)) y **clasifica cada
> una como regresión bloqueante o no bloqueante**, con su justificación.

## Por qué existe este documento

El Criterio 10.7 exige que, al completar la comparación funcional manual final,
Midway_Desktop cuente con **un reporte de diferencias encontradas** entre su
comportamiento y el de Midway_Tauri —si las hubiera— **clasificando cada diferencia
como regresión bloqueante o no bloqueante**. Este reporte alimenta directamente la
compuerta de bloqueo del Criterio 10.8 (verificada en la Tarea 17.4): la limpieza de
la Fase 8 (Tarea 18) **no** debe ejecutarse mientras exista una regresión
**bloqueante** sin documentar ni aceptar explícitamente.

## Definiciones de clasificación

- **Regresión bloqueante** — una diferencia de **comportamiento observable** por la
  que Midway_Desktop **pierde, degrada o altera** una funcionalidad respecto a
  Midway_Tauri para las mismas entradas, de forma que el usuario obtiene un resultado
  peor o incorrecto. Una regresión bloqueante **detiene** la compuerta hacia la Fase 8
  hasta ser resuelta o aceptada explícitamente por el responsable técnico (Criterio 10.8).
- **No bloqueante** — una diferencia que **no** altera el comportamiento observable de
  cara al usuario. Entran aquí: (a) diferencias de **mecanismo interno** (mismo
  resultado, distinta implementación), justificadas por la eliminación de la capa
  Tauri/navegador o por la ausencia de un crate publicado apto (Criterio 11.3); y
  (b) ajustes deliberados exigidos por los requisitos. Todas están respaldadas por la
  suite automatizada (28 Correctness Properties + unit/integración) y/o por la
  reutilización sin reescritura de `midway-core`.

## Método y fuente

Las diferencias enumeradas provienen de dos fuentes ya producidas en el spec:

1. La *Parte E.4 — Diferencias a escalar a la Tarea 17.3* de
   [`docs/functional-comparison.md`](./functional-comparison.md).
2. La tabla *Resumen de decisiones "resueltas distinto" y ausencias de crate* y las
   notas por fase de [`docs/functional-equivalence.md`](./functional-equivalence.md).

No se identificó **ninguna diferencia de comportamiento observable** en los cinco
flujos obligatorios del Criterio 10.6 (crear request, guardar request, correr una
colección, importar OpenAPI, exportar cURL). Por tanto, **no hay regresiones
bloqueantes**. Las diferencias existentes son de mecanismo o de ajuste deliberado, y
se clasifican todas como **no bloqueantes** con su justificación abajo.

---

## Tabla de clasificación de diferencias

| # | Área (Fase) | Diferencia observada | Midway_Tauri | Midway_Desktop | Clasificación | Justificación |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | Editor de texto (Fase 1) | Resaltado de sintaxis JSON | CodeMirror (`CodeEditor.tsx`) | `iced_highlighter` (crate publicado apto, Criterio 11.2) | **No bloqueante** | Solo cambia el widget de render; el contenido y el resaltado observable son equivalentes. Resta confirmación **visual** por tester humano (no es una regresión). |
| D2 | Editor de texto (Fase 1) | Búsqueda in-editor | Búsqueda provista por CodeMirror | Implementación manual sobre `Content` (case-insensitive, next/previous) | **No bloqueante** | Sin crate de búsqueda in-editor apto para `iced` (Criterio 11.3). Comportamiento equivalente anclado por la **Propiedad 7**. |
| D3 | Workspace panel — History (Fase 3) | Límite de entradas del historial | 50 (`commands/mod.rs`) | 500 (`HISTORY_LIMIT`) | **No bloqueante** | Ajuste **deliberado** exigido por el Req 4.6 (mejora, no pérdida). Orden por recencia y cota verificados por la **Propiedad 14**. |
| D4 | Workspace panel — Diagnostics (Fase 3) | Persistencia de crashes | `window.localStorage` | Archivo JSON en el data dir (máx. 200 `CrashRecord`) | **No bloqueante** | Sin navegador; persistencia nativa equivalente. Lista acotada y ordenada por recencia verificada por la **Propiedad 14** + unit tests. |
| D5 | Collection runner (Fase 4) | Transporte del progreso | `app.emit(COLLECTION_RUN_PROGRESS_EVENT, ...)` | Canal `mpsc` + `iced::subscription` | **No bloqueante** | Sin sistema de eventos de Tauri; la **semántica** de progreso (inicio/fin de request, contadores, reporte final) es equivalente. Anclado por la **Propiedad 18**. |
| D6 | Sesión (Fase 5) | Autosave de sesión | `localStorage` | `session.json` con escritura atómica (temporal + `rename`) en el data dir | **No bloqueante** | Sin `localStorage`; la garantía de ≤ 2 s desde la última modificación se preserva y se verifica con reloj simulado en la **Propiedad 19** (round-trip en la **Propiedad 20**). |
| D7 | Error boundary (Fase 5) | Aislamiento de fallos por componente | `componentDidCatch` de React | `std::panic::catch_unwind` por handler | **No bloqueante** | `iced` no tiene `componentDidCatch` y no hay crate equivalente (Criterio 11.3); el efecto observable (aislar el fallo, registrar `CrashRecord`, no derribar la app) es equivalente. Anclado por la **Propiedad 23**. |
| D8 | Updater in-app (Fase 6) | Motor de actualización | `tauri-plugin-updater` | Implementación manual sobre `reqwest` (+ `semver`, `sha2`) | **No bloqueante** | Sin crate compatible con el manifest/checksums ya publicados (Criterios 7.1 y 11.3). Consume **los mismos** artefactos sin modificarlos; verificar/descargar/validar SHA256/instalar/relanzar equivalente. Anclado por las **Propiedades 25–27**. |
| D9 | Empaquetado y distribución (Fase 7) | Herramienta de empaquetado | `tauri-build` / Tauri CLI | `cargo-packager` (versión pinneada, Criterio 8.1) | **No bloqueante** | Al retirar Tauri se requiere un empaquetador nativo de Rust. Genera los mismos instaladores (Windows NSIS/MSI, Linux AppImage/deb) y preserva `latest.json`/`latest-beta.json`/`SHA256SUMS.txt` (Req 8.3–8.5). |

**Total de diferencias clasificadas: 9 — todas no bloqueantes. Regresiones bloqueantes: 0.**

---

## Ítem de estado previamente abierto — ahora RESUELTO

Durante la Tarea 17.2, la *Parte E.4* de `docs/functional-comparison.md` y la *Nota de
estado* de la Fase 7 en `docs/functional-equivalence.md` escalaron un **ítem abierto**
(no una regresión de comportamiento):

| # | Ítem | Estado en Tarea 17.2 | Estado actual | Clasificación |
| --- | --- | --- | --- | --- |
| O1 | Cierre de la Fase 7 (empaquetado/distribución): detención del pipeline ante fallo (Tarea 15.5), smoke tests de CI (Tarea 15.6) y Checkpoint 16 | Abierto (tareas en curso) | **Resuelto** | **No bloqueante (resuelto)** |

**Actualización de estado (verificada en `tasks.md`):** las tareas **15.5** y **15.6**
y el **Checkpoint 16 (Fase 7)** están completadas (`[x]`), al igual que la Fase 7
completa (tarea 15). El workspace pasa `cargo check` y `cargo test` con la suite en
verde (167 tests). En consecuencia, el ítem **O1 ya no es un pendiente abierto**: la
detención del pipeline ante fallo de empaquetado (Req 8.6) y los smoke tests de
artefactos/updater (Req 8.2–8.4) están implementados y verificados. Este punto **deja
de ser un ítem bloqueante abierto** para la compuerta de la Tarea 17.4.

> La confirmación **de plataforma** de la Fase 7 que sigue siendo responsabilidad del
> tester humano (instalación en sistema limpio en Windows/Linux x86_64) es una
> verificación manual pendiente, **no** una regresión: la lógica de pipeline y la
> generación de artefactos ya están cubiertas por tests automatizados.

---

## Verificaciones manuales pendientes (no son regresiones)

Los siguientes aspectos permanecen `☐ Pendiente` en `docs/functional-comparison.md`
porque **ningún test automatizado puede sustituirlos**; requieren la ejecución humana
lado a lado. **No** constituyen diferencias ni regresiones: son confirmaciones
visuales/de plataforma aún no ejecutadas.

- Aspecto **visual** del resaltado JSON en pantalla (relacionado con D1).
- Diálogo de **archivo del SO** al importar/exportar (Parte A.4.6).
- **Portapapeles del SO** al copiar el comando cURL (Parte A.5.4).
- **Relanzamiento del proceso** tras instalar una actualización (Parte B.6.5).
- **Instalación en sistema limpio** en Windows/Linux x86_64 (Parte B.7.1–B.7.2).

Si durante esa verificación humana apareciera una diferencia de comportamiento
observable, deberá añadirse a la tabla de clasificación de este reporte y evaluarse
como potencial regresión bloqueante antes de la firma de la Parte D de
`docs/functional-comparison.md`.

---

## Resumen ejecutivo y veredicto

- **Diferencias de comportamiento observable (regresiones): 0.**
- **Diferencias no bloqueantes clasificadas: 9** (D1–D9), todas de mecanismo interno o
  ajuste deliberado, respaldadas por la suite automatizada (Correctness Properties
  1–28 + unit/integración) y/o por la reutilización sin reescritura de `midway-core`.
- **Regresiones bloqueantes: 0.**
- **Ítems abiertos previos: 0** (el único ítem abierto, O1 — cierre de la Fase 7 —
  quedó **resuelto**: tareas 15.5, 15.6 y Checkpoint 16 completadas; workspace verde
  en `cargo check` + `cargo test`, 167 tests).
- **Verificaciones manuales pendientes: 5**, de naturaleza visual/de plataforma; no son
  regresiones y no bloquean por sí mismas.

**Veredicto respecto a la compuerta del Criterio 10.8:** al no existir ninguna
**regresión bloqueante** sin documentar ni aceptar, y estando resuelto el único ítem
abierto de Fase 7, **no hay bloqueo por diferencias de comportamiento** hacia la
Fase 8. La autorización final de la compuerta corresponde a la **Tarea 17.4**, que debe
verificar además la conclusión de las verificaciones manuales pendientes descritas
arriba.

## Documentos relacionados

- Resumen escrito de equivalencia funcional (Tarea 17.1): [`docs/functional-equivalence.md`](./functional-equivalence.md).
- Comparación funcional manual final (Tarea 17.2): [`docs/functional-comparison.md`](./functional-comparison.md).
- Verificación de la compuerta de bloqueo (Tarea 17.4): [`docs/blocking-regression-gate.md`](./blocking-regression-gate.md).
- Distribución y release: [`docs/distribution.md`](./distribution.md).
